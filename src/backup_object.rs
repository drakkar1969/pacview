use std::cell::{RefCell, OnceCell};

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

use strum::FromRepr;

use crate::enum_traits::EnumExt;
use crate::pkg_object::PkgBackup;

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
    #[enum_value(name = "Access Denied")]
    Locked,
}

impl EnumExt for BackupStatus {}

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
        #[property(get, set, construct_only)]
        package: RefCell<String>,

        // Read only properties
        #[property(get = Self::file_hash, nullable)]
        file_hash: OnceCell<Option<String>>,

        #[property(get = Self::status, builder(BackupStatus::default()))]
        _status: RefCell<BackupStatus>,
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
        fn file_hash(&self) -> Option<String> {
            self.file_hash.get_or_init(|| {
                let filename = self.filename.borrow();

                alpm::compute_md5sum(filename.as_str()).ok()
            })
            .clone()
        }

        fn status(&self) -> BackupStatus {
            self.obj().file_hash()
                .map_or(BackupStatus::Locked, |file_hash| {
                    if file_hash == self.obj().hash() {
                        BackupStatus::Unmodified
                    } else {
                        BackupStatus::Modified
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
    pub fn new(backup: &PkgBackup) -> Self {
        glib::Object::builder()
            .property("filename", backup.filename())
            .property("hash", backup.hash())
            .property("package", backup.package())
            .build()
    }
}
