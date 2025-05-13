use gtk::glib;

use itertools::Itertools;

//------------------------------------------------------------------------------
// FLAGS: PkgFlags
//------------------------------------------------------------------------------
#[glib::flags(name = "PkgFlags")]
pub enum PkgFlags {
    ALL        = Self::INSTALLED.bits() | Self::NONE.bits(),
    INSTALLED  = Self::EXPLICIT.bits() | Self::DEPENDENCY.bits() | Self::OPTIONAL.bits() | Self::ORPHAN.bits(),
    EXPLICIT   = 0b0000_0001,
    DEPENDENCY = 0b0000_0010,
    OPTIONAL   = 0b0000_0100,
    ORPHAN     = 0b0000_1000,
    NONE       = 0b0001_0000,
    UPDATES    = 0b0010_0000,
}

impl Default for PkgFlags {
    fn default() -> Self {
        Self::empty()
    }
}

//------------------------------------------------------------------------------
// STRUCT: PkgData
//------------------------------------------------------------------------------
#[derive(Default, Debug)]
pub struct PkgData {
    pub flags: PkgFlags,
    pub name: String,
    pub version: String,
    pub description: String,
    pub popularity: String,
    pub out_of_date: i64,
    pub url: String,
    pub licenses: String,
    pub repository: String,
    pub groups: String,
    pub depends: Vec<String>,
    pub optdepends: Vec<String>,
    pub makedepends: Vec<String>,
    pub provides: Vec<String>,
    pub conflicts: Vec<String>,
    pub replaces: Vec<String>,
    pub architecture: String,
    pub packager: String,
    pub build_date: i64,
    pub install_date: i64,
    pub download_size: i64,
    pub install_size: i64,
    pub has_script: String,
    pub sha256sum: String,
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PkgData
//------------------------------------------------------------------------------
impl PkgData {
    //---------------------------------------
    // Alpm constructor
    //---------------------------------------
    pub fn from_alpm(sync_pkg: &alpm::Package, local_pkg: Option<&alpm::Package>, aur_names: &[String]) -> Self {
        // Helper functions
        fn alpm_list_to_string(list: alpm::AlpmList<&str>) -> String {
            list.iter().sorted_unstable().join(" | ")
        }

        fn alpm_deplist_to_vec(list: alpm::AlpmList<&alpm::Dep>) -> Vec<String> {
            list.iter().map(ToString::to_string).sorted_unstable().collect()
        }
        
        // Build PkgData
        let (flags, version, install_date) = local_pkg.map_or_else(
            || (PkgFlags::NONE, sync_pkg.version(), 0),
            |pkg| {
                let flags = match pkg.reason() {
                    alpm::PackageReason::Explicit => PkgFlags::EXPLICIT,
                    _ if !pkg.required_by().is_empty() => PkgFlags::DEPENDENCY,
                    _ if !pkg.optional_for().is_empty() => PkgFlags::OPTIONAL,
                    _ => PkgFlags::ORPHAN
                };

                (flags, pkg.version(), pkg.install_date().unwrap_or_default())
            }
        );

        let sync_name = sync_pkg.name();

        let repository = sync_pkg.db()
            .map_or("", |db| {
                let repo = db.name();

                if repo == "local" && aur_names.iter().any(|name| name == sync_name) {
                    "aur"
                } else {
                    repo
                }
            })
            .to_owned();

        Self {
            flags,
            name: sync_name.to_owned(),
            version: version.to_string(),
            description: sync_pkg.desc().unwrap_or_default().to_owned(),
            popularity: String::new(),
            out_of_date: 0,
            url: sync_pkg.url().unwrap_or_default().to_owned(),
            licenses: alpm_list_to_string(sync_pkg.licenses()),
            repository,
            groups: alpm_list_to_string(sync_pkg.groups()),
            depends: alpm_deplist_to_vec(sync_pkg.depends()),
            optdepends: alpm_deplist_to_vec(sync_pkg.optdepends()),
            makedepends: vec![],
            provides: alpm_deplist_to_vec(sync_pkg.provides()),
            conflicts: alpm_deplist_to_vec(sync_pkg.conflicts()),
            replaces: alpm_deplist_to_vec(sync_pkg.replaces()),
            architecture: sync_pkg.arch().unwrap_or_default().to_owned(),
            packager: sync_pkg.packager().unwrap_or("Unknown Packager").to_owned(),
            build_date: sync_pkg.build_date(),
            install_date,
            download_size: sync_pkg.download_size(),
            install_size: sync_pkg.isize(),
            has_script: if sync_pkg.has_scriptlet() { "Yes" } else { "No" }.to_owned(),
            sha256sum: sync_pkg.sha256sum().unwrap_or_default().to_owned(),
        }
    }

    //---------------------------------------
    // AUR constructor
    //---------------------------------------
    pub fn from_aur(pkg: &raur::Package) -> Self {
        // Helper functions
        fn aur_vec_to_string(slice: &[String]) -> String {
            slice.iter().sorted_unstable().join(" | ")
        }
        
        fn aur_sorted_vec(slice: &[String]) -> Vec<String> {
            slice.iter().map(String::from).sorted_unstable().collect()
        }

        // Build PkgData
        Self {
            flags: PkgFlags::NONE,
            name: pkg.name.clone(),
            version: pkg.version.clone(),
            description: pkg.description.clone().unwrap_or_default(),
            popularity: format!("{:.2?} ({} votes)", pkg.popularity, pkg.num_votes),
            out_of_date: pkg.out_of_date.unwrap_or_default(),
            url: pkg.url.clone().unwrap_or_default(),
            licenses: aur_vec_to_string(&pkg.license),
            repository: String::from("aur"),
            groups: aur_vec_to_string(&pkg.groups),
            depends: aur_sorted_vec(&pkg.depends),
            optdepends: aur_sorted_vec(&pkg.opt_depends),
            makedepends: aur_sorted_vec(&pkg.make_depends),
            provides: aur_sorted_vec(&pkg.provides),
            conflicts: aur_sorted_vec(&pkg.conflicts),
            replaces: aur_sorted_vec(&pkg.replaces),
            architecture: String::new(),
            packager: pkg.maintainer.clone().unwrap_or_else(|| String::from("Unknown Packager")),
            build_date: pkg.last_modified,
            install_date: 0,
            download_size: 0,
            install_size: 0,
            has_script: String::new(),
            sha256sum: String::new(),
        }
    }
}
