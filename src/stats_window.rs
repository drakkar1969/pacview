use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use size::Size;
use titlecase::titlecase;

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

            // Copy key binding
            klass.add_binding(gdk::Key::C, gdk::ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.copy_button.is_sensitive() {
                    imp.copy_button.emit_clicked();
                }

                glib::Propagation::Stop
            });
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
            obj.setup_controllers();
            obj.setup_signals();
        }
    }
    impl WidgetImpl for StatsWindow {}
    impl WindowImpl for StatsWindow {}
    impl AdwWindowImpl for StatsWindow {}
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
    // New function
    //---------------------------------------
    pub fn new(parent: &impl IsA<gtk::Window>) -> Self {
        glib::Object::builder()
            .property("transient-for", parent)
            .build()
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        // Set initial focus on view
        self.imp().view.grab_focus();
    }

    //---------------------------------------
    // Setup controllers
    //---------------------------------------
    fn setup_controllers(&self) {
        // Create shortcut controller
        let controller = gtk::ShortcutController::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);

        // Close window shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::NamedAction::new("window.close"))
        ));

        // Add shortcut controller to window
        self.add_controller(controller);
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = window)] self,
            #[weak] imp,
            move |_| {
                let body = imp.selection.iter::<glib::Object>().flatten()
                    .map(|item| {
                        let stat = item
                            .downcast::<StatsObject>()
                            .expect("Failed to downcast to 'StatsObject'");

                        format!("|{repository}|{packages}|{installed}|{size}|",
                            repository=stat.repository(),
                            packages=stat.packages(),
                            installed=stat.installed(),
                            size=stat.size())
                    })
                    .collect::<Vec<String>>()
                    .join("\n");

                window.clipboard().set_text(
                    &format!("## Package Statistics\n|Repository|Packages|Installed|Installed Size|\n|---|---|---|---|\n{body}")
                );
            }
        ));
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
    pub fn show(&self, repo_names: &[String]) {
        let imp = self.imp();

        self.present();

        // Populate if necessary
        if imp.model.n_items() == 0 {
            PKGS.with_borrow(|pkgs| {
                let mut stats_items: Vec<StatsObject> = vec![];

                // Iterate repos
                let (tot_pkg_count, tot_install_count, tot_install_size) = repo_names.iter()
                    .fold((0, 0, 0), |(tot_pkg_count, tot_install_count, tot_install_size), repo| {
                        // Iterate packages per repo
                        let (pkg_count, install_count, install_size) = pkgs.iter()
                            .filter(|pkg| pkg.repository() == *repo)
                            .fold((0, 0, 0), |(mut pkg_count, mut install_count, mut install_size), pkg| {
                                pkg_count += 1;

                                if pkg.flags().intersects(PkgFlags::INSTALLED) {
                                    install_count += 1;
                                    install_size += pkg.install_size();
                                }

                                (pkg_count, install_count, install_size)
                            });

                        // Add repo item to stats column view
                        let repo = if repo == "aur" { repo.to_uppercase() } else { titlecase(repo) };

                        stats_items.push(StatsObject::new(
                            &repo,
                            &pkg_count.to_string(),
                            &install_count.to_string(),
                            &Size::from_bytes(install_size).to_string()
                        ));

                        (tot_pkg_count + pkg_count, tot_install_count + install_count, tot_install_size + install_size)
                    });

                // Add item with totals to stats column view
                stats_items.push(StatsObject::new(
                    "<b>Total</b>",
                    &format!("<b>{tot_pkg_count}</b>"),
                    &format!("<b>{tot_install_count}</b>"),
                    &format!("<b>{}</b>", &Size::from_bytes(tot_install_size).to_string())
                ));

                imp.model.splice(0, 0, &stats_items);
            });
        }
    }
}
