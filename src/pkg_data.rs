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
// FLAGS: PkgValidation
//------------------------------------------------------------------------------
#[glib::flags(name = "PkgValidation")]
pub enum PkgValidation {
    UNKNOWN   = 0,
    NONE      = 1 << 0,
    #[flags_value(name = "MD5Sum")]
    MD5SUM    = 1 << 1,
    #[flags_value(name = "SHA256Sum")]
    SHA256SUM = 1 << 2,
    SIGNATURE = 1 << 3,
}

impl Default for PkgValidation {
    fn default() -> Self {
        Self::NONE
    }
}

//------------------------------------------------------------------------------
// STRUCT: PkgData
//------------------------------------------------------------------------------
#[derive(Default, Debug)]
pub struct PkgData {
    pub flags: PkgFlags,
    pub base: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub popularity: String,
    pub out_of_date: i64,
    pub url: String,
    pub licenses: Vec<String>,
    pub repository: String,
    pub groups: Vec<String>,
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
    pub validation: PkgValidation,
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PkgData
//------------------------------------------------------------------------------
impl PkgData {
    //---------------------------------------
    // Alpm constructor
    //---------------------------------------
    pub fn from_alpm(pkg: &alpm::Package, is_local: bool, repository: &str) -> Self {
        // Helper functions
        #[inline]
        fn alpm_list_to_vec(list: alpm::AlpmList<&str>) -> Vec<String> {
            list.iter().map(ToOwned::to_owned).sorted_unstable().collect()
        }

        #[inline]
        fn alpm_deplist_to_vec(list: alpm::AlpmList<&alpm::Dep>) -> Vec<String> {
            list.iter().map(ToString::to_string).sorted_unstable().collect()
        }

        // Build PkgData
        let flags = if is_local {
            match pkg.reason() {
                alpm::PackageReason::Explicit => PkgFlags::EXPLICIT,
                _ if !pkg.required_by().is_empty() => PkgFlags::DEPENDENCY,
                _ if !pkg.optional_for().is_empty() => PkgFlags::OPTIONAL,
                _ => PkgFlags::ORPHAN
            }
        } else {
            PkgFlags::NONE
        };

        Self {
            flags,
            base: pkg.base().map(String::from).unwrap_or_default(),
            name: pkg.name().to_owned(),
            version: pkg.version().to_string(),
            description: pkg.desc().map(String::from).unwrap_or_default(),
            popularity: String::new(),
            out_of_date: 0,
            url: pkg.url().map(String::from).unwrap_or_default(),
            licenses: alpm_list_to_vec(pkg.licenses()),
            repository: repository.to_owned(),
            groups: alpm_list_to_vec(pkg.groups()),
            depends: alpm_deplist_to_vec(pkg.depends()),
            optdepends: alpm_deplist_to_vec(pkg.optdepends()),
            makedepends: vec![],
            provides: alpm_deplist_to_vec(pkg.provides()),
            conflicts: alpm_deplist_to_vec(pkg.conflicts()),
            replaces: alpm_deplist_to_vec(pkg.replaces()),
            architecture: pkg.arch().map(String::from).unwrap_or_default(),
            packager: pkg.packager().unwrap_or("Unknown Packager").to_owned(),
            build_date: pkg.build_date(),
            install_date: pkg.install_date().unwrap_or_default(),
            download_size: pkg.download_size(),
            install_size: pkg.isize(),
            has_script: if pkg.has_scriptlet() { "Yes".into() } else { "No".into() },
            validation: PkgValidation::from_bits_truncate(pkg.validation().bits()),
        }
    }

    //---------------------------------------
    // AUR constructor
    //---------------------------------------
    pub fn from_aur(pkg: &raur::Package) -> Self {
        // Helper function
        #[inline]
        fn aur_sorted_vec(slice: &[String]) -> Vec<String> {
            slice.iter().map(String::from).sorted_unstable().collect()
        }

        // Build PkgData
        Self {
            flags: PkgFlags::NONE,
            base: pkg.package_base.clone(),
            name: pkg.name.clone(),
            version: pkg.version.clone(),
            description: pkg.description.as_deref().unwrap_or_default().to_owned(),
            popularity: format!("{:.2?} ({} votes)", pkg.popularity, pkg.num_votes),
            out_of_date: pkg.out_of_date.unwrap_or_default(),
            url: pkg.url.as_deref().unwrap_or_default().to_owned(),
            licenses: aur_sorted_vec(&pkg.license),
            repository: String::from("aur"),
            groups: aur_sorted_vec(&pkg.groups),
            depends: aur_sorted_vec(&pkg.depends),
            optdepends: aur_sorted_vec(&pkg.opt_depends),
            makedepends: aur_sorted_vec(&pkg.make_depends),
            provides: aur_sorted_vec(&pkg.provides),
            conflicts: aur_sorted_vec(&pkg.conflicts),
            replaces: aur_sorted_vec(&pkg.replaces),
            architecture: String::new(),
            packager: pkg.maintainer.as_deref().unwrap_or("Unknown Packager").to_owned(),
            build_date: pkg.last_modified,
            install_date: 0,
            download_size: 0,
            install_size: 0,
            has_script: String::new(),
            validation: PkgValidation::NONE,
        }
    }
}
