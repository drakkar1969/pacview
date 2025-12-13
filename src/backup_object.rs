use std::cell::{RefCell, OnceCell};
use std::marker::PhantomData;
use std::path::Path;
use std::fs;
use std::io;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

use strum::FromRepr;

use crate::window::{PACCAT_PATH, MELD_PATH};
use crate::enum_traits::EnumExt;
use crate::pkg_object::PkgBackup;
use crate::utils::async_command;

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
        status: OnceCell<BackupStatus>,

        #[property(get = Self::status_icon)]
        status_icon: PhantomData<String>,
        #[property(get = Self::status_text)]
        status_text: PhantomData<String>,
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
        // Property getters
        //---------------------------------------
        fn file_hash(&self) -> Option<String> {
            self.file_hash.get_or_init(|| {
                let filename = self.filename.borrow();

                alpm::compute_md5sum(filename.as_str()).ok()
            })
            .clone()
        }

        fn status(&self) -> BackupStatus {
            *self.status.get_or_init(|| {
                self.obj().file_hash()
                    .map_or(BackupStatus::Locked, |file_hash| {
                        if file_hash == self.obj().hash() {
                            BackupStatus::Unmodified
                        } else {
                            BackupStatus::Modified
                        }
                    })
            })
        }

        fn status_icon(&self) -> String {
            format!("backup-{}-symbolic", self.status().nick())
        }

        fn status_text(&self) -> String {
            self.status().name().to_lowercase()
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

    //---------------------------------------
    // Async compare with original function
    //---------------------------------------
    pub async fn compare_with_original(&self) -> io::Result<()> {
        let filename = self.filename();

        // Download original file with paccat
        let paccat_cmd = PACCAT_PATH.as_ref()
            .map_err(|_| io::Error::other("Paccat not found"))?;

        let (status, content) = async_command::run(paccat_cmd, &[&self.package(), "--", &filename]).await?;

        if status != Some(0) {
            return Err(io::Error::other("Paccat error"))
        }

        // Save original file to /tmp folder
        let tmp_filename = Path::new(&filename).file_name()
            .map(|file_name| format!("/tmp/{}.original", file_name.to_string_lossy()))
            .ok_or_else(|| io::Error::other("Failed to create temporary filename"))?;

        fs::write(&tmp_filename, content)?;

        // Compare file with original
        let meld_cmd = MELD_PATH.as_ref()
            .map_err(|_| io::Error::other("Meld not found"))?;

        let (status, _) = async_command::run(meld_cmd, &[&tmp_filename, &filename]).await?;

        if status == Some(0) {
            Ok(())
        } else {
            Err(io::Error::other("Meld error"))
        }
    }
}
