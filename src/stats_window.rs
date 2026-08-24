use std::cell::Cell;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use gdk::{Key, ModifierType};
use glib::clone;

use itertools::Itertools;
use size::Size;
use heck::ToTitleCase;

use crate::{
    pkg_object::PkgObject,
    stats_object::StatsObject
};

//------------------------------------------------------------------------------
// MODULE: StatsWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::StatsWindow)]
    #[template(resource = "/com/github/PacView/ui/stats_window.ui")]
    pub struct StatsWindow {
        #[template_child]
        pub(super) repo_grid: TemplateChild<gtk::Grid>,
        #[template_child]
        pub(super) total_grid: TemplateChild<gtk::Grid>,

        #[property(get, set)]
        is_loaded: Cell<bool>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for StatsWindow {
        const NAME: &'static str = "StatsWindow";
        type Type = super::StatsWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            StatsObject::ensure_type();

            klass.bind_template();

            // Add key bindings
            Self::bind_shortcuts(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for StatsWindow {}
    impl WidgetImpl for StatsWindow {}
    impl WindowImpl for StatsWindow {}
    impl AdwWindowImpl for StatsWindow {}

    impl StatsWindow {
        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &<Self as ObjectSubclass>::Class) {
            // Close window binding
            klass.add_binding_action(Key::Escape, ModifierType::NO_MODIFIER_MASK, "window.close");
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: StatsWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct StatsWindow(ObjectSubclass<imp::StatsWindow>)
        @extends adw::Window, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl StatsWindow {
    //---------------------------------------
    // Grid row helper function
    //---------------------------------------
    fn attach_grid_row(grid: &gtk::Grid, row: i32, has_icon: bool, repo: &str, size: i64, inst: usize, pkgs: usize) {
        // Attach image
        if has_icon {
            let image = gtk::Image::builder()
                .icon_name("repository-symbolic")
                .build();

            grid.attach(&image, 0, row, 1, 1);
        }

        // Attach repo name/size labels
        let repo_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .build();

        let repo_label = gtk::Label::builder()
            .xalign(0.0)
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

        if has_icon {
            grid.attach(&repo_box, 1, row, 1, 1);
        } else {
            grid.attach(&repo_box, 0, row, 2, 1);
        }

        // Attach progress bar and installed/pkgs labels
        let progress_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::End)
            .margin_start(24)
            .spacing(6)
            .build();

        let progress_bar = gtk::ProgressBar::builder()
            .valign(gtk::Align::Center)
            .fraction(if pkgs == 0 { 0.0 } else { inst as f64 / pkgs as f64 })
            .css_classes(["horizontal", "stats"])
            .build();

        let details_box = gtk::Box::builder()
            .spacing(32)
            .build();

        let inst_label = gtk::Label::builder()
            .xalign(0.0)
            .label(format!("{inst} installed"))
            .css_classes(["caption-heading", "dimmed"])
            .build();

        let pkgs_label = gtk::Label::builder()
            .hexpand(true)
            .xalign(1.0)
            .label(format!("{pkgs} packages"))
            .css_classes(["caption-heading", "dimmed"])
            .build();

        details_box.append(&inst_label);
        details_box.append(&pkgs_label);

        progress_box.append(&progress_bar);
        progress_box.append(&details_box);

        grid.attach(&progress_box, 2, row, 1, 1);

    }

    //---------------------------------------
    // Populate window
    //---------------------------------------
    fn populate(&self, repos: Vec<String>, pkg_model: &gio::ListStore) {
        glib::spawn_future_local(clone!(
            #[weak(rename_to = window)] self,
            #[weak] pkg_model,
            async move {
                let imp = window.imp();

                let mut pkgs_total = 0;
                let mut inst_total = 0;
                let mut size_total = 0;

                let pkg_list: Vec<(String, String, i64)> = pkg_model.iter::<PkgObject>()
                    .flatten()
                    .map(|pkg| (pkg.repository(), pkg.status().to_owned(), pkg.install_size()))
                    .collect();

                let mut row = 0;

                for repo in repos {
                    let map = pkg_list.iter()
                        .filter(|(repository, _, _)| repository == &repo)
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
                        Self::attach_grid_row(&imp.repo_grid, row, true, &repo, size, inst, pkgs);

                        row += 1;
                    }
                }

                Self::attach_grid_row(&imp.total_grid, row + 1, false, "Total", size_total, inst_total, pkgs_total);
            }
        ));
    }

    //---------------------------------------
    // Show window
    //---------------------------------------
    pub fn show(&self, repos: &[String], pkg_model: &gio::ListStore) {
        let repos = repos.to_owned();

        glib::idle_add_local_once(clone!(
            #[weak(rename_to = window)] self,
            #[weak] pkg_model,
            move || {
                if !window.is_loaded() {
                    window.populate(repos, &pkg_model);

                    window.set_is_loaded(true);
                }

                window.present();
            }
        ));
    }
}

impl Default for StatsWindow {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
