use ahqstore_types::{
  winget::Installer, AHQStoreApplication, AppRepo, DownloadUrl, InstallerFormat, InstallerOptions, InstallerOptionsWindows, WindowsInstallScope
};
use serde_yml::from_str;
use std::{
  collections::HashMap, fs::{self, File}, io::Write
};
use version_compare::Version;

mod http;
struct Map {
  entries: usize,
  files: usize,
  c_file: File,
  search: File,
}

impl Map {
  fn new() -> Self {
    let _ = fs::create_dir_all("./db/map");
    let _ = fs::create_dir_all("./db/search");
    let _ = fs::create_dir_all("./db/apps");
    let _ = fs::create_dir_all("./db/dev");
    let _ = fs::create_dir_all("./db/res");

    let mut file = File::create("./db/map/1.json").unwrap();
    let _ = file.write(b"{");

    let mut search = File::create("./db/search/1.json").unwrap();
    let _ = search.write(b"[");

    Self {
      entries: 0,
      files: 1,
      c_file: file,
      search,
    }
  }

  fn close_file(&mut self) {
    let _ = self.search.write_all(b"]");
    let _ = self.search.flush();
    let _ = self.c_file.write_all(b"}");
    let _ = self.c_file.flush();
  }

  fn new_file(&mut self) {
    self.files += 1;
    self.entries = 0;
    self.close_file();

    let mut map = File::create("./db/map/1.json").unwrap();
    let _ = map.write(b"{");

    let mut search = File::create("./db/map/1.json").unwrap();
    let _ = search.write(b"[");

    self.c_file = map;
    self.search = search;
  }

  fn add_author(&mut self, author: &str, app_id: &str) {
    let file = format!("./db/dev/{}", author);
    let mut val = fs::read_to_string(&file).unwrap_or("".to_string());
    val.push_str(&format!("{}\n", &app_id));

    let _ = fs::write(&file, val);
  }

  fn add(&mut self, mut app: AHQStoreApplication) {
    if self.entries >= 100_000 {
      self.new_file();
    }
    println!("{}", self.entries);
    if self.entries > 0 {
      let _ = self.c_file.write(b",");
      let _ = self.search.write(b",");
    }

    self.add_author(&app.authorId, &app.appId);
    self.entries += 1;

    let _ = self
      .c_file
      .write(format!("\"{}\":\"{}\"", app.appDisplayName, app.appId).as_bytes());
    let _ = self.search.write(
      format!(
        "{{\"name\": {:?}, \"title\": {:?}, \"id\": {:?}}}",
        fixstr(&app.appDisplayName),
        fixstr(&app.appShortcutName),
        fixstr(&app.appId)
      )
      .as_bytes(),
    );

    let (_, res) = app.export();

    app.resources = None;

    let app_str = serde_json::to_string(&app).unwrap();

    let app_export_path = format!("./db/apps/{}.json", &app.appId);

    let _ = fs::write(app_export_path, app_str);

    let _ = fs::create_dir_all(format!("./db/res/{}", &app.appId));

    for (id, bytes) in res {
      let _ = fs::write(format!("./db/res/{}/{}", &app.appId, id), bytes);
    }
  }

  fn finish(mut self) {
    self.close_file();

    let _ = fs::write("./db/total", self.files.to_string());
  }
}

fn fixstr(st: &str) -> String {
  st.replace("\u{a0}", " ")
}

pub async fn parser() {
  println!("⏲️ Please wait...");
  let _ = fs::remove_dir_all("./db");
  let _ = fs::create_dir_all("./db");

  let mut map = Map::new();

  for letter in fs::read_dir("./winget-pkgs/manifests").unwrap() {
    let letter = letter.unwrap().file_name();
    let letter = letter.to_str().unwrap();

    for author in fs::read_dir(format!("./winget-pkgs/manifests/{}", &letter)).unwrap() {
      let author = author.unwrap().file_name();
      let author = author.to_str().unwrap();

      app_parse(letter, author, &mut map).await;
    }
  }
  map.finish();
  println!("✅ Done!");
}

use async_recursion::async_recursion;

#[async_recursion]
async fn app_parse(letter: &str, author: &str, map: &mut Map) {
  for app in fs::read_dir(format!("./winget-pkgs/manifests/{}/{}", &letter, &author)).unwrap() {
    let app = app.unwrap();

    if !app.file_type().unwrap().is_dir() {
      continue;
    }

    let app = app.file_name();

    let app = app.to_str().unwrap();

    if app == ".validation" {
      continue;
    }

    let inside = fs::read_dir(format!(
      "./winget-pkgs/manifests/{}/{}/{}",
      &letter, &author, &app
    ))
    .unwrap()
    .into_iter();

    let inside = inside
      .map(|x| x.unwrap())
      .filter(|x| x.file_type().unwrap().is_dir())
      .map(|x| x.file_name())
      .collect::<Vec<_>>();
    let inside = inside.into_iter();
    let inside = inside.filter(|x| x != ".validation").collect::<Vec<_>>();
    let inside = inside.into_iter();

    let versions = inside
      .clone()
      .filter(|x| Version::from(x.to_str().unwrap_or("unknown")).is_some())
      .collect::<Vec<_>>();

    let mut latest: Option<String> = None;

    versions.into_iter().for_each(|version| {
      let version = version.to_str();
      let version = version.unwrap_or("0.0.0");
      let ver_string = version.to_string();
      let version = Version::from(ver_string.as_str()).unwrap();

      if let Some(latest) = latest.as_mut() {
        if version > Version::from(latest).unwrap() {
          *latest = ver_string;
        } else {
          drop(version);
          drop(ver_string);
        }
      } else {
        latest = Some(ver_string);
      }
    });

    if latest.is_some() {
      let v = latest.unwrap();

      let letter = author.trim();
      let mut letter = letter.split("").collect::<Vec<_>>();
      let letter = letter.remove(1);
      let letter = letter.to_lowercase();

      let app_id = format!("{}.{}", &author.replace("/", "."), &app);
      let en_us =
        format!("./winget-pkgs/manifests/{letter}/{author}/{app}/{v}/{app_id}.locale.en-US.yaml");
      let installer =
        format!("./winget-pkgs/manifests/{letter}/{author}/{app}/{v}/{app_id}.installer.yaml");

      if let (Ok(en_us), Ok(installer)) =
        (fs::read_to_string(&en_us), fs::read_to_string(&installer))
      {
        use ahqstore_types::winget::{InstallerScheme, WingetApplication};

        if let (Ok(en_us), Ok(installer)) = (
          from_str::<WingetApplication>(&en_us),
          from_str::<InstallerScheme>(&installer),
        ) {
          let mut arm = DownloadUrl {
            asset: "".into(),
            installerType: InstallerFormat::WindowsInstallerMsi,
            url: "".into(),
          };
          let mut x64 = DownloadUrl {
            asset: "".into(),
            installerType: InstallerFormat::WindowsInstallerMsi,
            url: "".into(),
          };

          let mut win32 = None;
          let mut winarm = None;

          let mut msi: (Option<Installer>, Option<Installer>) = (None, None);
          let mut exe: (Option<Installer>, Option<Installer>) = (None, None);

          for x in installer.Installers {
            let mut type_msi = x.InstallerUrl.ends_with(".msi");
            let mut type_exe = x.InstallerUrl.ends_with(".exe");

            if !type_msi && !type_exe {
              type_msi = http::cnt_dsp_check(&x.InstallerUrl, ".msi").await;
              type_exe = http::cnt_dsp_check(&x.InstallerUrl, ".exe").await;
            }

            let locale = x.InstallerLocale.clone();
            let arch = &x.Architecture;

            if &locale.unwrap_or("en-US".into()) == "en-US" {
              if type_msi && &arch == &"x64" {
                msi.0 = Some(x);
              } else if type_msi && &arch == &"arm64" {
                msi.1 = Some(x);
              } else if type_exe && &arch == &"x64" {
                exe.0 = Some(x);
              } else if type_exe && &arch == &"arm64" {
                exe.1 = Some(x);
              }
            }
          }

          let x64_dat = msi.0.map_or_else(
            || exe.0.and_then(|x| Some((InstallerFormat::WindowsInstallerExe, x))),
            |x| Some((InstallerFormat::WindowsInstallerMsi, x)),
          );
          let arm64_dat = msi.1.map_or_else(
            || exe.1.and_then(|x| Some((InstallerFormat::WindowsInstallerExe, x))),
            |x| Some((InstallerFormat::WindowsInstallerMsi, x)),
          );

          let scope = installer.Scope;

          let mut parse = |x: Option<(InstallerFormat, Installer)>| if let Some((installer, x)) = x {
            let scope = match scope.as_str() {
              "machine" => WindowsInstallScope::Machine,
              "user" => WindowsInstallScope::User,
              _ => WindowsInstallScope::User,
            };

            if &x.Architecture == "x64" {
              x64.installerType = installer;

              x64.url = x.InstallerUrl;

              win32 = Some(InstallerOptionsWindows {
                assetId: 1,
                exec: None,
                installerArgs: None,
                scope: Some(scope),
              });
            } else if &x.Architecture == "arm64" {
              arm.installerType = installer;

              arm.url = x.InstallerUrl;

              winarm = Some(InstallerOptionsWindows {
                assetId: 0,
                exec: None,
                installerArgs: None,
                scope: Some(scope),
              });
            }
          };

          parse(x64_dat);
          parse(arm64_dat);

          let app = AHQStoreApplication {
            appDisplayName: en_us.PackageName,
            appId: format!("winget_app_{}", app_id.replace("-", "_")),
            appShortcutName: format!("Winget Application"),
            authorId: format!("winget"),
            description: format!(
              "{}\n\n{}\n{}",
              en_us.ShortDescription.unwrap_or_default(),
              en_us.Description.unwrap_or_default(),
              en_us.ReleaseNotes.unwrap_or_default()
            ),
            displayImages: vec![],
            releaseTagName: format!("winget-{}", v),
            version: v,
            source: en_us.Publisher,
            license_or_tos: en_us.License,
            repo: AppRepo {
              author: "microsoft".into(),
              repo: "winget-pkgs".into(),
            },
            site: en_us.PublisherUrl,
            resources: Some({
              let mut data = HashMap::new();
              data.insert(0, include_bytes!("../../icon.png").to_vec());
              data
            }),
            downloadUrls: {
              let mut data = HashMap::new();
              data.insert(0, arm);
              data.insert(1, x64);
              data
            },
            install: InstallerOptions {
              android: None,
              linux: None,
              linuxArm64: None,
              linuxArm7: None,
              win32,
              winarm,
            },
            verified: false,
          };

          println!("✅ Added {author} {app_id}");

          map.add(app);
        } else {
          println!("⚠️ Unable to parse {author} {app}");
        }
      } else {
        println!("⚠️ Cancelled Author: {author} App: {app} Ver: {v:?}");
      }
    }

    for product in inside.filter(|x| Version::from(x.to_str().unwrap_or("unknown")).is_none()) {
      if product.to_str().unwrap_or(".yaml").ends_with(".yaml") {
        continue;
      }

      app_parse(
        letter,
        &format!("{author}/{app}/{}", product.to_str().unwrap_or("unknown")),
        map,
      ).await;
    }
  }
}
