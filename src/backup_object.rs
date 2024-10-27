use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

use strum::FromRepr;

use crate::enum_traits::EnumValueExt;

//------------------------------------------------------------------------------
// ENUM: BackupStatus
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum, FromRepr)]
#[repr(u32)]
#[enum_type(name = "BackupStatus")]
pub enum BackupStatus {
    All,
    Modified,
    Unmodified,
    #[default]
    #[enum_value(name = "Read Error")]
    Error,
}

impl EnumValueExt for BackupStatus {}

//------------------------------------------------------------------------------
// MODULE: BackupObject
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
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

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for BackupObject {
        const NAME: &'static str = "BackupObject";
        type Type = super::BackupObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for BackupObject {}

    impl BackupObject {
        //---------------------------------------
        // Custom property getters
        //---------------------------------------
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
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(filename: &str, hash: &str, package: Option<&str>, file_hash: &str) -> Self {
        let status = if !file_hash.is_empty() {
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
}
