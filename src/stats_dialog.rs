use std::cell::Cell;

use gtk::{glib, gio, pango};
use adw::subclass::prelude::*;
use adw::prelude::*;

use itertools::Itertools;
use size::Size;
use heck::ToTitleCase;

use crate::{
    pkg_object::PkgObject,
    stats_object::StatsObject
};

//------------------------------------------------------------------------------
// MODULE: StatsDialog
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::StatsDialog)]
    #[template(resource = "/com/github/PacView/ui/stats_dialog.ui")]
    pub struct StatsDialog {
        #[template_child]
        pub(super) repo_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub(super) total_group: TemplateChild<adw::PreferencesGroup>,

        #[property(get, set)]
        is_loaded: Cell<bool>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for StatsDialog {
        const NAME: &'static str = "StatsDialog";
        type Type = super::StatsDialog;
        type ParentType = adw::PreferencesDialog;

        fn class_init(klass: &mut Self::Class) {
            StatsObject::ensure_type();

            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for StatsDialog {}
    impl WidgetImpl for StatsDialog {}
    impl AdwDialogImpl for StatsDialog {}
    impl PreferencesDialogImpl for StatsDialog {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: StatsDialog
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct StatsDialog(ObjectSubclass<imp::StatsDialog>)
        @extends adw::PreferencesDialog, adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl StatsDialog {
    //---------------------------------------
    // Add row helper function
    //---------------------------------------
    fn add_row(&self, is_total: bool, repo: &str, size: i64, inst: usize, pkgs: usize) {
        let imp = self.imp();

        // Create row widget
        let row_box = gtk::Box::builder()
            .margin_start(20)
            .margin_end(20)
            .margin_top(12)
            .margin_bottom(12)
            .spacing(10)
            .build();

        // Append image
        if !is_total {
            let image = gtk::Image::builder()
                .icon_name("repository-symbolic")
                .build();

            row_box.append(&image);
        }

        // Append repo name/size labels
        let repo_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::End)
            .spacing(2)
            .build();

        let repo_label = gtk::Label::builder()
            .xalign(0.0)
            .max_width_chars(12)
            .ellipsize(pango::EllipsizeMode::End)
            .label(if repo == "aur" { repo.to_uppercase() } else { repo.to_title_case() })
            .css_classes(["heading"])
            .build();

        let size_label = gtk::Label::builder()
            .xalign(0.0)
            .label(Size::from_bytes(size).to_string())
            .css_classes(["caption-heading", "dimmed"])
            .build();

        repo_box.append(&repo_label);
        repo_box.append(&size_label);

        row_box.append(&repo_box);

        // Append progress bar and installed/pkgs labels
        let info_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::End)
            .margin_start(24)
            .spacing(6)
            .build();

        let progress_bar = gtk::ProgressBar::builder()
            .fraction(if pkgs == 0 { 0.0 } else { inst as f64 / pkgs as f64 })
            .css_classes(["horizontal", "stats"])
            .build();

        let details_box = gtk::Box::builder()
            .spacing(32)
            .build();

        let inst_label = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(pango::EllipsizeMode::End)
            .label(format!("{inst} installed"))
            .css_classes(["caption-heading", "dimmed"])
            .build();

        let pkgs_label = gtk::Label::builder()
            .hexpand(true)
            .xalign(1.0)
            .ellipsize(pango::EllipsizeMode::End)
            .label(format!("{pkgs} pkgs"))
            .css_classes(["caption-heading", "dimmed"])
            .build();

        details_box.append(&inst_label);
        details_box.append(&pkgs_label);

        info_box.append(&progress_bar);
        info_box.append(&details_box);

        row_box.append(&info_box);

        // Create action row
        let row = adw::ActionRow::builder()
            .build();

        row.set_child(Some(&row_box));

        if is_total {
            imp.total_group.add(&row);
        } else {
            imp.repo_group.add(&row);
        }
    }

    //---------------------------------------
    // Populate dialog
    //---------------------------------------
    fn populate(&self, repos: &[String], pkg_model: &gio::ListStore) {
        let mut pkgs_total = 0;
        let mut inst_total = 0;
        let mut size_total = 0;

        let pkg_list: Vec<(String, String, i64)> = pkg_model.iter::<PkgObject>()
            .flatten()
            .map(|pkg| (pkg.repository(), pkg.status().to_owned(), pkg.install_size()))
            .collect();

        for repo in repos {
            let map = pkg_list.iter()
                .filter(|(repository, _, _)| repository == repo)
                .into_group_map_by(|(_, status, _)| status);

            let pkgs: usize = map.values()
                .map(Vec::len)
                .sum();

            let (inst, size): (usize, i64) = map.iter()
                .filter(|&(&key, _)| !key.is_empty())
                .map(|(_, value)| {
                    let inst = value.len();
                    let size = value.iter()
                        .map(|(_, _, size)| *size)
                        .sum();

                    (inst, size)
                })
                .reduce(|(acc_inst, acc_size), (inst, size)| {
                    (acc_inst + inst, acc_size + size)
                })
                .unwrap_or_default();

            // Update total counts
            pkgs_total += pkgs;
            inst_total += inst;
            size_total += size;

            if pkgs > 0 {
                self.add_row(false, &repo, size, inst, pkgs);
            }
        }

        self.add_row(true, "Total", size_total, inst_total, pkgs_total);
    }

    //---------------------------------------
    // Show dialog
    //---------------------------------------
    pub fn show(&self, parent: Option<&impl IsA<gtk::Widget>>, repos: &[String], pkg_model: &gio::ListStore) {
        let parent = parent.map(|widget| widget.clone().upcast());

        if !self.is_loaded() {
            self.populate(repos, &pkg_model);

            self.set_is_loaded(true);
        }

        self.present(parent.as_ref());
    }
}

impl Default for StatsDialog {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
