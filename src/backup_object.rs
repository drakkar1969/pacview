use std::cell::{RefCell, OnceCell};

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
        // Read-write properties, construct only
        #[property(get, set, construct_only)]
        filename: RefCell<String>,
        #[property(get, set, construct_only)]
        hash: RefCell<String>,
        #[property(get, set, nullable, construct_only)]
        package: RefCell<Option<String>>,

        // Read only properties
        #[property(get = Self::status, builder(BackupStatus::default()))]
        status: OnceCell<BackupStatus>,
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
        fn status(&self) -> BackupStatus {
            *self.status.get_or_init(|| {
                let file_hash = alpm::compute_md5sum(self.obj().filename())
                    .unwrap_or_default();

                if !file_hash.is_empty() {
                    if file_hash == self.obj().hash() {
                        BackupStatus::Unmodified
                    } else {
                        BackupStatus::Modified
                    }
                } else {
                    BackupStatus::Error
                }
            })
        }

        fn status_icon(&self) -> String {
            format!("backup-{}-symbolic", self.status().nick())
        }

        fn status_text(&self) -> String {
            self.status().name().to_ascii_lowercase()
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
    pub fn new(filename: &str, hash: &str, package: Option<&str>) -> Self {
        glib::Object::builder()
            .property("filename", filename)
            .property("hash", hash)
            .property("package", package)
            .build()
    }
}
