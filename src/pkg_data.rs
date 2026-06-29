use std::fmt::Display;

use gtk::glib;

use itertools::Itertools;
use alpm::PackageReason;

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

impl Display for PkgValidation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let flags_class = glib::FlagsClass::new::<Self>();

        let display = self.iter()
            .map(|flag| {
                flags_class
                    .value(flag.bits())
                    .map_or("None", glib::FlagsValue::name)
            })
            .collect::<Vec<&str>>()
            .join(" | ");

        write!(f, "{display}")
    }
}

impl PkgValidation {
    pub fn is_valid(self) -> bool {
        !(self.intersects(Self::UNKNOWN) || self.intersects(Self::NONE))
    }
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
    pub is_installed: bool,
    pub base: Option<String>,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub popularity: Option<String>,
    pub out_of_date: Option<i64>,
    pub url: Option<String>,
    pub licenses: Vec<String>,
    pub repository: String,
    pub groups: Vec<String>,
    pub depends: Vec<String>,
    pub optdepends: Vec<String>,
    pub makedepends: Vec<String>,
    pub provides: Vec<String>,
    pub conflicts: Vec<String>,
    pub replaces: Vec<String>,
    pub architecture: Option<String>,
    pub packager: Option<String>,
    pub build_date: i64,
    pub install_date: Option<i64>,
    pub download_size: i64,
    pub install_size: i64,
    pub has_script: Option<String>,
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
        fn list_to_vec(list: alpm::AlpmList<&str>) -> Vec<String> {
            list.iter().map(ToOwned::to_owned).sorted_unstable().collect()
        }

        #[inline]
        fn deplist_to_vec(list: alpm::AlpmList<&alpm::Dep>) -> Vec<String> {
            list.iter().map(ToString::to_string).sorted_unstable().collect()
        }

        // Build PkgData
        let flags = if is_local {
            match pkg.reason() {
                PackageReason::Explicit => PkgFlags::EXPLICIT,
                PackageReason::Depend if !pkg.required_by().is_empty() => PkgFlags::DEPENDENCY,
                PackageReason::Depend if !pkg.optional_for().is_empty() => PkgFlags::OPTIONAL,
                PackageReason::Depend => PkgFlags::ORPHAN
            }
        } else {
            PkgFlags::NONE
        };

        Self {
            flags,
            is_installed: is_local,
            base: pkg.base().map(ToOwned::to_owned),
            name: pkg.name().to_owned(),
            version: pkg.version().to_string(),
            description: pkg.desc().map(ToOwned::to_owned),
            popularity: None,
            out_of_date: None,
            url: pkg.url().map(ToOwned::to_owned),
            licenses: list_to_vec(pkg.licenses()),
            repository: repository.to_owned(),
            groups: list_to_vec(pkg.groups()),
            depends: deplist_to_vec(pkg.depends()),
            optdepends: deplist_to_vec(pkg.optdepends()),
            makedepends: vec![],
            provides: deplist_to_vec(pkg.provides()),
            conflicts: deplist_to_vec(pkg.conflicts()),
            replaces: deplist_to_vec(pkg.replaces()),
            architecture: pkg.arch().map(ToOwned::to_owned),
            packager: pkg.packager().map(ToOwned::to_owned),
            build_date: pkg.build_date(),
            install_date: pkg.install_date(),
            download_size: pkg.download_size(),
            install_size: pkg.isize(),
            has_script: pkg.has_scriptlet().then(|| "Yes".into()),
            validation: PkgValidation::from_bits_truncate(pkg.validation().bits()),
        }
    }

    //---------------------------------------
    // AUR constructor
    //---------------------------------------
    pub fn from_aur(pkg: &raur::Package) -> Self {
        // Helper function
        #[inline]
        fn sorted_vec(slice: &[String]) -> Vec<String> {
            slice.iter().map(String::from).sorted_unstable().collect()
        }

        println!("{:?}", pkg.package_base);

        // Build PkgData
        Self {
            flags: PkgFlags::NONE,
            is_installed: false,
            base: None,
            name: pkg.name.clone(),
            version: pkg.version.clone(),
            description: pkg.description.clone(),
            popularity: Some(format!("{:.2?} ({} vote{})", pkg.popularity, pkg.num_votes, if pkg.num_votes == 1 { "" } else { "s" })),
            out_of_date: pkg.out_of_date,
            url: pkg.url.clone(),
            licenses: sorted_vec(&pkg.license),
            repository: "aur".into(),
            groups: sorted_vec(&pkg.groups),
            depends: sorted_vec(&pkg.depends),
            optdepends: sorted_vec(&pkg.opt_depends),
            makedepends: sorted_vec(&pkg.make_depends),
            provides: sorted_vec(&pkg.provides),
            conflicts: sorted_vec(&pkg.conflicts),
            replaces: sorted_vec(&pkg.replaces),
            architecture: None,
            packager: pkg.maintainer.clone(),
            build_date: pkg.last_modified,
            install_date: None,
            download_size: 0,
            install_size: 0,
            has_script: None,
            validation: PkgValidation::NONE,
        }
    }
}
