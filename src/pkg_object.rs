use std::cell::{RefCell, OnceCell};
use std::rc::Rc;
use std::cmp::Ordering;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;
use glib::clone;

use alpm_utils::DbListExt;
use regex::Regex;
use glob::glob;
use size::Size;
use rayon::prelude::*;

use crate::window::{PACMAN_CONFIG, PACMAN_LOG, PKGS, INSTALLED_PKGS, INSTALLED_PKG_NAMES};
use crate::pkg_data::{PkgFlags, PkgData};

//------------------------------------------------------------------------------
// GLOBAL VARIABLES
//------------------------------------------------------------------------------
thread_local! {
    pub static ALPM_HANDLE: RefCell<Option<Rc<alpm::Alpm>>> = const { RefCell::new(None) };
}

//------------------------------------------------------------------------------
// STRUCT: PkgBackup
//------------------------------------------------------------------------------
#[derive(Default, Debug, Clone)]
pub struct PkgBackup {
    filename: String,
    hash: String,
    package: String
}

impl PkgBackup {
    fn new(filename: &str, hash: &str, package: &str) -> Self {
        Self {
            filename: filename.to_owned(),
            hash: hash.to_owned(),
            package: package.to_owned()
        }
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn package(&self) -> &str {
        &self.package
    }
}

//------------------------------------------------------------------------------
// MODULE: PkgObject
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::PkgObject)]
    pub struct PkgObject {
        // Alpm handle
        pub(super) handle: OnceCell<Rc<alpm::Alpm>>,

        // Read-write properties
        #[property(get, set, nullable)]
        update_version: RefCell<Option<String>>,

        // Read-only properties with custom getter
        #[property(name = "flags", get = Self::flags, type = PkgFlags)]
        #[property(name = "version", get = Self::version, type = String)]
        #[property(name = "show-version-icon", get = Self::show_version_icon, type = bool)]
        #[property(name = "status", get = Self::status, type = String)]
        #[property(name = "status-icon", get = Self::status_icon, type = String)]
        #[property(name = "status-icon-symbolic", get = Self::status_icon_symbolic, type = String)]
        #[property(name = "show-status-icon", get = Self::show_status_icon, type = bool)]
        #[property(name = "install-size-string", get = Self::install_size_string, type = String)]
        #[property(name = "show-groups-icon", get = Self::show_groups_icon, type = bool)]

        // Read-only properties from data fields
        #[property(name = "name", get, type = String, member = name)]
        #[property(name = "repository", get, type = String, member = repository)]
        #[property(name = "install-size", get, type = i64, member = install_size)]
        #[property(name = "groups", get, type = String, member = groups)]
        pub(super) data: OnceCell<PkgData>,

        // Read only fields
        pub(super) package_url: OnceCell<String>,
        pub(super) out_of_date_string: OnceCell<String>,
        pub(super) install_date_string: OnceCell<String>,
        pub(super) build_date_string: OnceCell<String>,
        pub(super) download_size_string: OnceCell<String>,

        pub(super) required_by: OnceCell<Vec<String>>,
        pub(super) optional_for: OnceCell<Vec<String>>,

        pub(super) files: OnceCell<Vec<String>>,
        pub(super) log: OnceCell<Vec<String>>,
        pub(super) cache: OnceCell<Vec<String>>,
        pub(super) backup: OnceCell<Vec<PkgBackup>>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PkgObject {
        const NAME: &'static str = "PkgObject";
        type Type = super::PkgObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for PkgObject {}

    impl PkgObject {
        //---------------------------------------
        // Read-only property getters
        //---------------------------------------
        fn flags(&self) -> PkgFlags {
            let flags = self.data.get().unwrap().flags;

            self.obj().update_version()
                .map_or_else(|| flags, |_| flags | PkgFlags::UPDATES)
        }

        fn version(&self) -> String {
            let version = &self.data.get().unwrap().version;

            self.obj().update_version()
                .map_or_else(|| version.to_string(), |update_version|
                    version.to_string() + " \u{2192} " + &update_version
                )
        }

        fn show_version_icon(&self) -> bool {
            self.flags().intersects(PkgFlags::UPDATES)
        }

        fn status(&self) -> String {
            match self.data.get().unwrap().flags {
                PkgFlags::EXPLICIT => "explicit",
                PkgFlags::DEPENDENCY => "dependency",
                PkgFlags::OPTIONAL => "optional",
                PkgFlags::ORPHAN => "orphan",
                _ => "not installed"
            }.to_owned()
        }

        fn status_icon(&self) -> String {
            match self.data.get().unwrap().flags {
                PkgFlags::EXPLICIT => "pkg-explicit",
                PkgFlags::DEPENDENCY => "pkg-dependency",
                PkgFlags::OPTIONAL => "pkg-optional",
                PkgFlags::ORPHAN => "pkg-orphan",
                _ => ""
            }.to_owned()
        }

        fn status_icon_symbolic(&self) -> String {
            match self.data.get().unwrap().flags {
                PkgFlags::EXPLICIT => "status-explicit-symbolic",
                PkgFlags::DEPENDENCY => "status-dependency-symbolic",
                PkgFlags::OPTIONAL => "status-optional-symbolic",
                PkgFlags::ORPHAN => "status-orphan-symbolic",
                _ => ""
            }.to_owned()
        }

        fn show_status_icon(&self) -> bool {
            self.data.get().unwrap().flags.intersects(PkgFlags::INSTALLED)
        }

        fn install_size_string(&self) -> String {
            Size::from_bytes(self.data.get().unwrap().install_size).to_string()
        }

        fn show_groups_icon(&self) -> bool {
            !self.data.get().unwrap().groups.is_empty()
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PkgObject
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PkgObject(ObjectSubclass<imp::PkgObject>);
}

impl PkgObject {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(data: PkgData, handle: Option<Rc<alpm::Alpm>>) -> Self {
        let pkg: Self = glib::Object::builder()
            .build();

        let imp = pkg.imp();

        imp.data.set(data).unwrap();

        if let Some(handle) = handle {
            imp.handle.set(handle).unwrap();
        }

        pkg.connect_update_version_notify(|pkg| {
            pkg.notify_flags();
            pkg.notify_version();
            pkg.notify_show_version_icon();
        });

        pkg
    }

    //---------------------------------------
    // Public data field getters
    //---------------------------------------
    pub fn description(&self) -> &str {
        &self.imp().data.get().unwrap().description
    }

    pub fn popularity(&self) -> &str {
        &self.imp().data.get().unwrap().popularity
    }

    pub fn out_of_date(&self) -> i64 {
        self.imp().data.get().unwrap().out_of_date
    }

    pub fn out_of_date_string(&self) -> &str {
        self.imp().out_of_date_string.get_or_init(|| {
            Self::date_to_string(self.imp().data.get().unwrap().out_of_date, "%d %B %Y %H:%M")
        })
    }

    pub fn package_url(&self) -> &str {
        self.imp().package_url.get_or_init(|| {
            let default_repos = ["core", "extra", "multilib"];

            let data = self.imp().data.get().unwrap();

            let repo = &data.repository;

            if default_repos.contains(&repo.as_str()) {
                format!("https://www.archlinux.org/packages/{repo}/{arch}/{name}",
                    arch=data.architecture,
                    name=data.name
                )
            } else if repo == "aur" {
                format!("https://aur.archlinux.org/packages/{name}",
                    name=data.name
                )
            } else {
                String::new()
            }
        })
    }

    pub fn url(&self) -> &str {
        &self.imp().data.get().unwrap().url
    }

    pub fn licenses(&self) -> &str {
        &self.imp().data.get().unwrap().licenses
    }

    pub fn depends(&self) -> &[String] {
        &self.imp().data.get().unwrap().depends
    }

    pub fn optdepends(&self) -> &[String] {
        &self.imp().data.get().unwrap().optdepends
    }

    pub fn makedepends(&self) -> &[String] {
        &self.imp().data.get().unwrap().makedepends
    }

    pub fn provides(&self) -> &[String] {
        &self.imp().data.get().unwrap().provides
    }

    pub fn conflicts(&self) -> &[String] {
        &self.imp().data.get().unwrap().conflicts
    }

    pub fn replaces(&self) -> &[String] {
        &self.imp().data.get().unwrap().replaces
    }

    pub fn architecture(&self) -> &str {
        &self.imp().data.get().unwrap().architecture
    }

    pub fn packager(&self) -> &str {
        &self.imp().data.get().unwrap().packager
    }

    pub fn install_date(&self) -> i64 {
        self.imp().data.get().unwrap().install_date
    }

    pub fn install_date_string(&self) -> &str {
        self.imp().install_date_string.get_or_init(|| {
            Self::date_to_string(self.imp().data.get().unwrap().install_date, "%d %B %Y %H:%M")
        })
    }

    pub fn build_date(&self) -> i64 {
        self.imp().data.get().unwrap().build_date
    }

    pub fn build_date_string(&self) -> &str {
        self.imp().build_date_string.get_or_init(|| {
            Self::date_to_string(self.imp().data.get().unwrap().build_date, "%d %B %Y %H:%M")
        })
    }

    pub fn download_size(&self) -> i64 {
        self.imp().data.get().unwrap().download_size
    }

    pub fn download_size_string(&self) -> &str {
        self.imp().download_size_string.get_or_init(|| {
            Size::from_bytes(self.imp().data.get().unwrap().download_size).to_string()
        })
    }

    pub fn has_script(&self) -> &str {
        &self.imp().data.get().unwrap().has_script
    }

    pub fn sha256sum(&self) -> &str {
        &self.imp().data.get().unwrap().sha256sum
    }

    //---------------------------------------
    // Get alpm package helper function
    //---------------------------------------
    fn pkg(&self) -> Option<&alpm::Package> {
        let imp = self.imp();

        let handle = imp.handle.get()?;
        let data = imp.data.get().unwrap();

        if data.flags.intersects(PkgFlags::INSTALLED) {
            handle.localdb().pkg(data.name.as_str()).ok()
        } else {
            handle.syncdbs().pkg(data.name.as_str()).ok()
        }
    }

    //---------------------------------------
    // Public getters from alpm package
    //---------------------------------------
    pub fn required_by(&self) -> &[String] {
        self.imp().required_by.get_or_init(|| {
            self.pkg()
                .map(|pkg| {
                    let mut required_by: Vec<String> = pkg.required_by().into_iter()
                        .par_bridge()
                        .collect();

                    required_by.par_sort_unstable();

                    required_by
                })
                .unwrap_or_default()
        })
    }

    pub fn optional_for(&self) -> &[String] {
        self.imp().optional_for.get_or_init(|| {
            self.pkg()
                .map(|pkg| {
                    let mut optional_for: Vec<String> = pkg.optional_for().into_iter()
                        .par_bridge()
                        .collect();

                    optional_for.par_sort_unstable();

                    optional_for
                })
                .unwrap_or_default()
        })
    }

    pub fn files(&self) -> &[String] {
        let imp = self.imp();

        imp.files.get_or_init(|| {
            self.pkg()
                .map(|pkg| {
                    let root_dir = &PACMAN_CONFIG.get().unwrap().root_dir;

                    let mut files: Vec<String> = pkg.files().files().iter()
                        .par_bridge()
                        .map(|file| root_dir.to_string() + file.name())
                        .collect();

                    files.par_sort_unstable();

                    files
                })
                .unwrap_or_default()
        })
    }

    pub fn backup(&self) -> &[PkgBackup] {
        let imp = self.imp();

        imp.backup.get_or_init(|| {
            self.pkg()
                .map(|pkg| {
                    let root_dir = &PACMAN_CONFIG.get().unwrap().root_dir;
                    let pkg_name = self.name();

                    let mut backup: Vec<PkgBackup> = pkg.backup().iter()
                        .par_bridge()
                        .map(|backup|
                            PkgBackup::new(&(root_dir.to_string() + backup.name()), backup.hash(), &pkg_name)
                        )
                        .collect();

                    backup.par_sort_unstable_by(|backup_a, backup_b|
                        backup_a.filename.partial_cmp(&backup_b.filename).unwrap_or(Ordering::Equal)
                    );

                    backup
                })
                .unwrap_or_default()
        })
    }

    //---------------------------------------
    // Public async functions
    //---------------------------------------
    pub fn log_async<F>(&self, f: F)
    where F: Fn(&[String]) + 'static {
        let imp = self.imp();

        if let Some(log) = imp.log.get() {
            f(log);
        } else {
            let pkg_name = self.name();

            let (sender, receiver) = async_channel::bounded(1);

            PACMAN_LOG.with_borrow(|pacman_log| {
                gio::spawn_blocking(clone!(
                    #[strong] pacman_log,
                    move || {
                        let log: Vec<String> = pacman_log.map_or(vec![], |pacman_log| {
                            let expr = Regex::new(&format!(r"\[(.+?)T(.+?)\+.+?\] \[ALPM\] (installed|removed|upgraded|downgraded) ({name}) (.+)", name=regex::escape(&pkg_name)))
                                .expect("Regex error");

                            pacman_log.lines().rev()
                                .filter(|s| s.contains(&pkg_name))
                                .filter(|&s|
                                    expr.is_match(s))
                                        .map(|s| expr.replace(s, "[$1  $2] : $3 $4 $5").into_owned())
                                .collect()
                        });

                        sender.send_blocking(log).expect("Failed to send through channel");
                    }
                ));
            });

            glib::spawn_future_local(clone!(
                #[weak] imp,
                async move {
                    while let Ok(log) = receiver.recv().await {
                        f(&log);

                        imp.log.set(log).unwrap();
                    }
                }
            ));
        }
    }

    pub fn cache_async<F>(&self, f: F)
    where F: Fn(&[String]) + 'static {
        let imp = self.imp();

        if let Some(cache) = imp.cache.get() {
            f(cache);
        } else {
            let pkg_name = self.name();

            let (sender, receiver) = async_channel::bounded(1);

            gio::spawn_blocking(clone!(
                move || {
                    let pacman_config = PACMAN_CONFIG.get().unwrap();

                    let cache: Vec<String> = pacman_config.cache_dir.iter()
                        .flat_map(|dir| {
                            glob(&format!("{dir}{pkg_name}*.pkg.tar.zst"))
                                .expect("Glob pattern error")
                                .flatten()
                                .filter_map(|path|
                                    path.file_name()
                                        .and_then(|filename| filename.to_str())
                                        .filter(|filename| 
                                            filename.rsplitn(4, '-').last()
                                                .is_some_and(|name| name == pkg_name)
                                        )
                                        .map(ToOwned::to_owned)
                                )
                        })
                        .collect();

                    sender.send_blocking(cache).expect("Failed to send through channel");
                }
            ));

            glib::spawn_future_local(clone!(
                #[weak] imp,
                async move {
                    while let Ok(cache) = receiver.recv().await {
                        f(&cache);

                        imp.cache.set(cache).unwrap();
                    }
                }
            ));
        }
    }

    //---------------------------------------
    // Date to string associated function
    //---------------------------------------
    pub fn date_to_string(date: i64, format: &str) -> String {
        if date == 0 {
            String::new()
        } else {
            glib::DateTime::from_unix_local(date)
                .and_then(|datetime| datetime.format(format))
                .expect("Datetime error")
                .to_string()
        }
    }

    //---------------------------------------
    // Satisfier associated functions
    //---------------------------------------
    pub fn has_local_satisfier(search_term: &str) -> Option<bool> {
        ALPM_HANDLE.with_borrow(|alpm_handle| {
            let handle = alpm_handle.as_ref()?;

            handle.localdb().pkgs().find_satisfier(search_term)
                .map(|local_pkg|
                    INSTALLED_PKG_NAMES.with_borrow(|installed_pkg_names|
                        installed_pkg_names.contains(local_pkg.name())
                    )
                )
        })
    }

    pub fn find_satisfier(search_term: &str) -> Option<Self> {
        ALPM_HANDLE.with_borrow(|alpm_handle| {
            let handle = alpm_handle.as_ref()?;

            handle.localdb().pkgs().find_satisfier(search_term)
                .and_then(|local_pkg|
                    INSTALLED_PKGS.with_borrow(|installed_pkgs|
                        installed_pkgs.iter()
                            .find(|&pkg| pkg.name() == local_pkg.name())
                            .cloned()
                    )
                )
                .or_else(|| {
                    handle.syncdbs().find_satisfier(search_term)
                        .and_then(|sync_pkg|
                            PKGS.with_borrow(|pkgs|
                                pkgs.iter()
                                    .find(|&pkg| pkg.name() == sync_pkg.name())
                                    .cloned()
                            )
                        )
                })
        })
    }
}
