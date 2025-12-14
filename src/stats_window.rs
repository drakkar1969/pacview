use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use gdk::{Key, ModifierType};

use size::Size;
use heck::ToTitleCase;

use crate::window::PKGS;
use crate::pkg_data::PkgFlags;
use crate::stats_object::StatsObject;

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
        pub(super) copy_button: TemplateChild<gtk::Button>,

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

            obj.setup_signals();
            obj.setup_widgets();
        }
    }
    impl WidgetImpl for StatsWindow {}
    impl WindowImpl for StatsWindow {}
    impl AdwWindowImpl for StatsWindow {}

    impl StatsWindow {
        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Close window binding
            klass.add_binding_action(Key::Escape, ModifierType::NO_MODIFIER_MASK, "window.close");

            // Copy key binding
            klass.add_binding(Key::C, ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.copy_button.is_sensitive() {
                    imp.copy_button.emit_clicked();
                }

                glib::Propagation::Stop
            });
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
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = window)] self,
            move |_| {
                let body = window.imp().selection.iter::<glib::Object>().flatten()
                    .map(|item| {
                        let stat = item
                            .downcast::<StatsObject>()
                            .expect("Failed to downcast to 'StatsObject'");

                        format!("|{repository}|{packages}|{installed}|{explicit}|{size}|",
                            repository=stat.repository(),
                            packages=stat.packages(),
                            installed=stat.installed(),
                            explicit=stat.explicit(),
                            size=stat.size())
                    })
                    .collect::<Vec<String>>()
                    .join("\n");

                window.clipboard().set_text(
                    &format!("## Package Statistics\n|Repository|Packages|Installed|Explicit|Installed Size|\n|---|---|---|---|---|\n{body}")
                );
            }
        ));
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        // Set initial focus on view
        self.imp().view.grab_focus();
    }

    //---------------------------------------
    // Clear window
    //---------------------------------------
    pub fn remove_all(&self) {
        self.imp().model.remove_all();
    }

    //---------------------------------------
    // Show window
    //---------------------------------------
    pub fn show(&self, parent: &impl IsA<gtk::Window>, repos: &[String]) {
        let imp = self.imp();

        self.set_transient_for(Some(parent));
        self.present();

        // Populate if necessary
        if imp.model.n_items() == 0 {
            PKGS.with_borrow(|pkgs| {
                let mut stats_items: Vec<StatsObject> = Vec::with_capacity(repos.len() + 1);

                let mut pkg_count_total = 0;
                let mut install_count_total = 0;
                let mut install_size_total = 0;
                let mut explicit_count_total = 0;

                // Iterate repos
                for repo in repos {
                    let mut pkg_count = 0;
                    let mut install_count = 0;
                    let mut install_size = 0;
                    let mut explicit_count = 0;

                    // Iterate packages in repo
                    for pkg in pkgs.iter().filter(|pkg| &pkg.repository() == repo) {
                        pkg_count += 1;

                        if pkg.flags().intersects(PkgFlags::INSTALLED) {
                            install_count += 1;
                            install_size += pkg.install_size();
                        }

                        if pkg.flags().intersects(PkgFlags::EXPLICIT) {
                            explicit_count += 1;
                        }
                    }

                    pkg_count_total += pkg_count;
                    install_count_total += install_count;
                    install_size_total += install_size;
                    explicit_count_total += explicit_count;

                    // Add repo item to stats view
                    stats_items.push(StatsObject::new(
                        Some("repository-symbolic"),
                        &(if *repo == "aur" { repo.to_uppercase() } else { repo.to_title_case() }),
                        &pkg_count.to_string(),
                        &install_count.to_string(),
                        &explicit_count.to_string(),
                        &Size::from_bytes(install_size).to_string()
                    ));
                }

                // Add item with totals to stats view
                stats_items.push(StatsObject::new(
                    None,
                    "<b>Total</b>",
                    &format!("<b>{pkg_count_total}</b>"),
                    &format!("<b>{install_count_total}</b>"),
                    &format!("<b>{explicit_count_total}</b>"),
                    &format!("<b>{}</b>", &Size::from_bytes(install_size_total).to_string())
                ));

                imp.model.splice(0, 0, &stats_items);
            });
        }
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
