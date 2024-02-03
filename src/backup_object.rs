use std::cell::RefCell;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

use crate::pkg_object::PkgBackup;

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
        status_icon: RefCell<String>,
        #[property(get, set)]
        status_text: RefCell<String>,
        #[property(get, set, nullable)]
        package: RefCell<Option<String>>,
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
    pub fn new(backup: &PkgBackup, package: Option<&str>) -> Self {
        let (status_icon, status_text) = if let Ok(file_hash) = alpm::compute_md5sum(backup.filename.to_string()) {
            if file_hash == backup.hash {
                ("backup-unmodified-symbolic", "unmodified")
            } else {
                ("backup-modified-symbolic", "modified")
            }
        } else {
            ("backup-error-symbolic", "read error")
        };

        // Build BackupObject
        glib::Object::builder()
            .property("filename", &backup.filename)
            .property("status-icon", status_icon)
            .property("status-text", status_text)
            .property("package", package)
            .build()
    }
}
