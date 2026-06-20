use std::fmt::Write as _;

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
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/stats_window.ui")]
    pub struct StatsWindow {
        #[template_child]
        pub(super) view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::NoSelection>,
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

            // Install actions
            Self::install_actions(klass);

            // Add key bindings
            Self::bind_shortcuts(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for StatsWindow {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
        }
    }
    impl WidgetImpl for StatsWindow {}
    impl WindowImpl for StatsWindow {}
    impl AdwWindowImpl for StatsWindow {}

    impl StatsWindow {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Copy action
            klass.install_action("stats.copy", None, |window, _, _| {
                let mut output = String::from("## Package Statistics\n|Repository|Packages|Installed|Explicit|Installed Size|\n|---|---|---|---|---|\n");

                for stat in window.imp().selection.iter::<glib::Object>()
                    .flatten()
                    .filter_map(|item| item.downcast::<StatsObject>().ok()) {
                        writeln!(output,
                            "|{repository}|{packages}|{installed}|{explicit}|{size}|",
                            repository=stat.repository(),
                            packages=stat.packages(),
                            installed=stat.installed(),
                            explicit=stat.explicit(),
                            size=stat.size()
                        )
                        .unwrap();
                    }

                window.clipboard().set_text(&output);
            });
        }

        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Close window binding
            klass.add_binding_action(Key::Escape, ModifierType::NO_MODIFIER_MASK, "window.close");

            // Copy key binding
            klass.add_binding_action(Key::C, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "stats.copy");
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
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        // Set initial focus on view
        self.imp().view.grab_focus();
    }

    //---------------------------------------
    // Populate window
    //---------------------------------------
    pub fn populate(&self, repos: &[String], pkg_model: &gio::ListStore) {
        let imp = self.imp();

        let repos = repos.to_owned();

        glib::spawn_future_local(clone!(
            #[weak] imp,
            #[weak] pkg_model,
            async move {
                let mut pkg_count_total = 0;
                let mut install_count_total = 0;
                let mut install_size_total = 0;
                let mut explicit_count_total = 0;

                let pkg_list: Vec<(String, String, i64)> = pkg_model.iter::<PkgObject>()
                    .flatten()
                    .map(|pkg| (pkg.repository(), pkg.status().to_owned(), pkg.install_size()))
                    .collect();

                // Build stats list per repo
                let mut stats_items: Vec<StatsObject> = repos.iter()
                    .map(|repo| {
                        let map = pkg_list.iter()
                            .filter(|(repository, _, _)| repository == repo)
                            .into_group_map_by(|(_, status, _)| status);

                        let pkg_count: usize = map.values()
                            .map(Vec::len)
                            .sum();

                        let (install_count, install_size) = map.iter()
                            .filter(|&(&key, _)| !key.is_empty())
                            .map(|(_, value)| {
                                (value.len(), value.iter().map(|(_, _, size)| *size).sum::<i64>())
                            })
                            .reduce(|(acc_n, acc_size), (n, size)| (acc_n + n, acc_size + size))
                            .unwrap_or_default();

                        let explicit_count: usize = map.iter()
                            .filter(|&(&key, _)| key == "explicit")
                            .map(|(_, value)| value.len())
                            .sum();

                        // Update total counts
                        pkg_count_total += pkg_count;
                        install_count_total += install_count;
                        install_size_total += install_size;
                        explicit_count_total += explicit_count;

                        // Add repo item to stats view
                        StatsObject::new(
                            Some("repository-symbolic"),
                            &(if repo == "aur" { repo.to_uppercase() } else { repo.to_title_case() }),
                            &pkg_count.to_string(),
                            &install_count.to_string(),
                            &explicit_count.to_string(),
                            &Size::from_bytes(install_size).to_string()
                        )
                    })
                    .collect();

                // Add item with totals to stats view
                stats_items.push(StatsObject::new(
                    None,
                    "<b>Total</b>",
                    &format!("<b>{pkg_count_total}</b>"),
                    &format!("<b>{install_count_total}</b>"),
                    &format!("<b>{explicit_count_total}</b>"),
                    &format!("<b>{}</b>", Size::from_bytes(install_size_total))
                ));

                imp.model.splice(0, imp.model.n_items(), &stats_items);
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
