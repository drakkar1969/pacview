use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

use crate::traits::{EnumValueExt, EnumClassExt};

//------------------------------------------------------------------------------
// ENUM: BackupStatus
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "BackupStatus")]
pub enum BackupStatus {
    Modified = 0,
    Unmodified = 1,
    #[default]
    #[enum_value(name = "Read Error")]
    Error = 2,
}

impl EnumValueExt for BackupStatus {}
impl EnumClassExt for BackupStatus {}

//------------------------------------------------------------------------------
// STRUCT: BackupData
//------------------------------------------------------------------------------
pub struct BackupData {
    filename: String,
    hash: String,
    package: Option<String>,
    file_hash: Result<String, alpm::ChecksumError>
}

impl BackupData {
    pub fn new(filename: &str, hash: &str, package: Option<&str>) -> Self {
        Self {
            filename: filename.to_string(),
            hash: hash.to_string(),
            package: package.map(str::to_string),
            file_hash: alpm::compute_md5sum(filename)
        }
    }
}

//------------------------------------------------------------------------------
// MODULE: BackupObject
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::BackupObject)]
    pub struct BackupObject {
        #[property(get, set)]
        filename: RefCell<String>,
        #[property(get, set)]
        hash: RefCell<String>,
        #[property(get, set, nullable)]
        package: RefCell<Option<String>>,
        #[property(get, set, builder(BackupStatus::default()))]
        status: Cell<BackupStatus>,

        #[property(get = Self::status_icon)]
        _status_icon: RefCell<String>,
        #[property(get = Self::status_text)]
        _status_text: RefCell<String>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for BackupObject {
        const NAME: &'static str = "BackupObject";
        type Type = super::BackupObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for BackupObject {}

    impl BackupObject {
        //-----------------------------------
        // Custom property getters
        //-----------------------------------
        fn status_icon(&self) -> String {
            format!("backup-{}-symbolic", self.obj().status().nick())
        }

        fn status_text(&self) -> String {
            self.obj().status().name().to_ascii_lowercase()
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: BackupObject
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct BackupObject(ObjectSubclass<imp::BackupObject>);
}

impl BackupObject {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(filename: &str, hash: &str, package: Option<&str>, file_hash: Result<&str, &alpm::ChecksumError>) -> Self {
        let status = if let Ok(file_hash) = file_hash {
            if file_hash == hash {
                BackupStatus::Unmodified
            } else {
                BackupStatus::Modified
            }
        } else {
            BackupStatus::Error
        };

        // Build BackupObject
        glib::Object::builder()
            .property("filename", filename)
            .property("hash", hash)
            .property("package", package)
            .property("status", status)
            .build()
    }

    //-----------------------------------
    // From data function
    //-----------------------------------
    pub fn from_data(data: &BackupData) -> Self {
        Self::new(&data.filename, &data.hash, data.package.as_deref(), data.file_hash.as_deref())
    }
}
