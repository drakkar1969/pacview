use gtk::glib;

use itertools::Itertools;

//------------------------------------------------------------------------------
// GLOBAL: Helper functions
//------------------------------------------------------------------------------
fn alpm_list_to_string(list: alpm::AlpmList<&str>) -> String {
    list.iter()
        .sorted_unstable()
        .join(" | ")
}

fn alpm_deplist_to_vec(list: alpm::AlpmList<&alpm::Dep>) -> Vec<String> {
    list.iter()
        .map(|dep| dep.to_string())
        .sorted_unstable()
        .collect()
}

fn aur_vec_to_string(vec: &[String]) -> String {
    vec.iter()
        .sorted_unstable()
        .join(" | ")
}

fn aur_sorted_vec(vec: &[String]) -> Vec<String> {
    vec.iter()
        .map(String::from)
        .sorted_unstable()
        .collect()
}

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
    pub has_script: bool,
    pub sha256sum: String,
}

//---------------------------------------
// Constructors
//---------------------------------------
impl PkgData {
    pub fn from_pkg(pkg: &alpm::Package, local_pkg: Option<&alpm::Package>, aur_names: &[String]) -> Self {
        let mut flags = PkgFlags::NONE;
        let mut install_date = 0i64;

        if let Some(pkg) = local_pkg {
            flags = if pkg.reason() == alpm::PackageReason::Explicit {
                PkgFlags::EXPLICIT
            } else {
                if !pkg.required_by().is_empty() {
                    PkgFlags::DEPENDENCY
                } else {
                    if !pkg.optional_for().is_empty() {
                        PkgFlags::OPTIONAL
                    } else {
                        PkgFlags::ORPHAN
                    }
                }
            };

            install_date = pkg.install_date().unwrap_or_default();
        }

        let repository = pkg.db()
            .map(|db| {
                let mut repo = db.name();

                if repo == "local" && aur_names.contains(&pkg.name().to_string()) {
                    repo = "aur";
                }

                repo.to_string()
            })
            .unwrap_or_default();

        Self {
            flags,
            name: pkg.name().to_string(),
            version: pkg.version().to_string(),
            description: pkg.desc().map(String::from).unwrap_or_default(),
            url: pkg.url().map(String::from).unwrap_or_default(),
            licenses: alpm_list_to_string(pkg.licenses()),
            repository,
            groups: alpm_list_to_string(pkg.groups()),
            depends: alpm_deplist_to_vec(pkg.depends()),
            optdepends: alpm_deplist_to_vec(pkg.optdepends()),
            makedepends: vec![],
            provides: alpm_deplist_to_vec(pkg.provides()),
            conflicts: alpm_deplist_to_vec(pkg.conflicts()),
            replaces: alpm_deplist_to_vec(pkg.replaces()),
            architecture: pkg.arch().map(String::from).unwrap_or_default(),
            packager: pkg.packager().map(String::from).unwrap_or(String::from("Unknown Packager")),
            build_date: pkg.build_date(),
            install_date,
            download_size: pkg.download_size(),
            install_size: pkg.isize(),
            has_script: pkg.has_scriptlet(),
            sha256sum: pkg.sha256sum().map(String::from).unwrap_or_default(),
        }
    }

    pub fn from_aur(pkg: &raur::Package) -> Self {
        Self {
            flags: PkgFlags::NONE,
            name: pkg.name.to_string(),
            version: pkg.version.to_string(),
            description: pkg.description.as_ref().map(String::from).unwrap_or_default(),
            url: pkg.url.as_ref().map(String::from).unwrap_or_default(),
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
            packager: pkg.maintainer.as_ref().map(String::from).unwrap_or(String::from("Unknown Packager")),
            build_date: pkg.last_modified,
            install_date: 0,
            download_size: 0,
            install_size: 0,
            has_script: false,
            sha256sum: String::new(),
        }
    }
}
