use std::cell::RefCell;

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use fancy_regex::Regex;
use lazy_static::lazy_static;
use url::Url;

use crate::window::PacViewWindow;
use crate::value_row::ValueRow;
use crate::pkg_object::{PkgObject, PkgFlags};
use crate::prop_object::PropObject;

//------------------------------------------------------------------------------
// MODULE: InfoPane
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::InfoPane)]
    #[template(resource = "/com/github/PacView/ui/info_pane.ui")]
    pub struct InfoPane {
        #[template_child]
        pub view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub value_factory: TemplateChild<gtk::SignalListItemFactory>,
        #[template_child]

        pub toolbar: TemplateChild<gtk::Box>,
        #[template_child]
        pub navbutton_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub prev_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub next_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub empty_label: TemplateChild<gtk::Label>,

        #[property(get, set)]
        main_window: RefCell<Option<PacViewWindow>>,

        #[property(get, set)]
        history_model: RefCell<gio::ListStore>,
        #[property(get, set)]
        history_selection: RefCell<gtk::SingleSelection>,

        #[property(get = Self::pkg)]
        _pkg: RefCell<Option<PkgObject>>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for InfoPane {
        const NAME: &'static str = "InfoPane";
        type Type = super::InfoPane;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            PropObject::ensure_type();

            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for InfoPane {
        //-----------------------------------
        // Default property functions
        //-----------------------------------
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
            obj.setup_signals();
        }
    }

    impl WidgetImpl for InfoPane {}
    impl BinImpl for InfoPane {}

    impl InfoPane {
        //-----------------------------------
        // Custom property getters
        //-----------------------------------
        fn pkg(&self) -> Option<PkgObject> {
            self.obj().history_selection().selected_item()
                .and_downcast::<PkgObject>()
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: InfoPane
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct InfoPane(ObjectSubclass<imp::InfoPane>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl InfoPane {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        // Hide info pane header
        if let Some(list_header) = self.imp().view.first_child() {
            if list_header.type_().name() == "GtkListItemWidget" {
                list_header.set_visible(false);
            }
        }

        // Initialize history model/selection
        self.set_history_model(gio::ListStore::new(PkgObject::static_type()));
        self.set_history_selection(gtk::SingleSelection::new(Some(self.history_model())));
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Value factory setup signal
        imp.value_factory.connect_setup(|_, item| {
            let value_row = ValueRow::new();

            item
                .downcast_ref::<gtk::ListItem>()
                .expect("Must be a 'ListItem'")
                .set_child(Some(&value_row));
        });

        // Value factory bind signal
        imp.value_factory.connect_bind(clone!(@weak self as obj => move |_, item| {
            let prop_obj = item
                .downcast_ref::<gtk::ListItem>()
                .expect("Must be a 'ListItem'")
                .item()
                .and_downcast::<PropObject>()
                .expect("Must be a 'PropObject'");

            let value_row = item
                .downcast_ref::<gtk::ListItem>()
                .expect("Must be a 'ListItem'")
                .child()
                .and_downcast::<ValueRow>()
                .expect("Must be a 'ValueRow'");

            value_row.bind_properties(&prop_obj);

            let label = &value_row.imp().label;

            let signal = label.connect_activate_link(clone!(@weak obj => @default-return gtk::Inhibit(true), move |_, link| obj.link_handler(link)));

            value_row.add_label_signal(signal);
        }));

        // Value factory unbind signal
        imp.value_factory.connect_unbind(|_, item| {
            let value_row = item
                .downcast_ref::<gtk::ListItem>()
                .expect("Must be a 'ListItem'")
                .child()
                .and_downcast::<ValueRow>()
                .expect("Must be a 'ValueRow'");

            value_row.unbind_properties();
            value_row.drop_label_signal();
        });
    }

    //-----------------------------------
    // Value label link handler
    //-----------------------------------
    fn link_handler(&self, link: &str) -> gtk::Inhibit {
        if let Ok(url) = Url::parse(link) {
            if url.scheme() == "pkg" {
                if let Some(pkg_name) = url.domain() {
                    let main_window = self.main_window().unwrap();

                    let pkg_model = main_window.imp().package_view.imp().model.get();

                    // Find link package by name
                    let mut new_pkg = pkg_model.iter::<PkgObject>().flatten().find(|pkg| pkg.name() == pkg_name);

                    // If link package is none, find by provides
                    if new_pkg.is_none() {
                        new_pkg = pkg_model.iter::<PkgObject>().flatten().find(|pkg| {
                            pkg.provides().iter().any(|s| s.contains(&pkg_name))
                        });
                    }

                    // If link package found
                    if let Some(new_pkg) = new_pkg {
                        let hist_model = self.history_model();
                        let hist_sel = self.history_selection();

                        // If link package is in infopane history, select it
                        if let Some(i) = hist_model.find(&new_pkg) {
                            hist_sel.set_selected(i);
                        } else {
                            // If link package is not in history, get current history package
                            let hist_index = hist_sel.selected();

                            // If history package is not the last one in history, truncate history list
                            if hist_index < hist_model.n_items() - 1 {
                                hist_model.splice(hist_index + 1, hist_model.n_items() - hist_index - 1, &Vec::<glib::Object>::new());
                            }

                            // Add link package to history
                            hist_model.append(&new_pkg);

                            // Update history selection to link package
                            hist_sel.set_selected(hist_index + 1);
                        }

                        // Display link package
                        self.display_package(Some(&new_pkg));
                    }
                }

                // Link handled
                return gtk::Inhibit(true)
            } 
        }

        // Link not handled (use default handler)
        gtk::Inhibit(false)
    }

    //-----------------------------------
    // Public display functions
    //-----------------------------------
    pub fn display_package(&self, pkg: Option<&PkgObject>) {
        let imp = self.imp();

        let hist_sel = self.history_selection();

        // Set infopane toolbar visibility
        imp.toolbar.set_visible(pkg.is_some());

        // Clear infopane
        imp.model.remove_all();

        // If package is not none, display it
        if let Some(pkg) = pkg {
            // Set infopane previous/next box visibility
            imp.navbutton_box.set_visible(hist_sel.n_items() > 1);

            // Set infopane prev/next button states
            imp.prev_button.set_sensitive(hist_sel.selected() > 0);
            imp.next_button.set_sensitive(hist_sel.n_items() > 0 && hist_sel.selected() < hist_sel.n_items() - 1);

            // Get package required by and optional for
            let main_window = self.main_window().unwrap();

            let handle = main_window.imp().alpm_handle.borrow();

            let (required_by, optional_for) = pkg.compute_requirements(&handle);

            // Name
            imp.model.append(&PropObject::new(
                "Name", &format!("<b>{}</b>", pkg.name()), None
            ));
            // Version
            imp.model.append(&PropObject::new(
                "Version", &pkg.version(), if pkg.has_update() {Some("pkg-update")} else {None}
            ));
            // Description
            imp.model.append(&PropObject::new(
                "Description", &self.prop_to_esc_string(&pkg.description()), None
            ));
            // Package URL
            let mut url = String::from("None");

            if main_window.imp().pacman_config.borrow().default_repos.contains(&pkg.repo_show()) {
                url = self.prop_to_esc_url(&format!("https://www.archlinux.org/packages/{repo}/{arch}/{name}", repo=pkg.repo_show(), arch=pkg.architecture(), name=pkg.name()));
            } else if &pkg.repo_show() == "aur" {
                url = self.prop_to_esc_url(&format!("https://aur.archlinux.org/packages/{name}", name=pkg.name()))
            }

            imp.model.append(&PropObject::new(
                "Package URL", &url, None
            ));
            // URL
            if pkg.url() != "" {
                imp.model.append(&PropObject::new(
                    "URL", &self.prop_to_esc_url(&pkg.url()), None
                ));
            }
            // Licenses
            if pkg.licenses() != "" {
                imp.model.append(&PropObject::new(
                    "Licenses", &self.prop_to_esc_string(&pkg.licenses()), None
                ));
            }
            // Status
            let status = &pkg.status();
            let status_icon = pkg.status_icon();

            imp.model.append(&PropObject::new(
                "Status", if pkg.flags().intersects(PkgFlags::INSTALLED) {&status} else {"not installed"}, if pkg.flags().intersects(PkgFlags::INSTALLED) {Some(&status_icon)} else {None}
            ));
            // Repository
            imp.model.append(&PropObject::new(
                "Repository", &pkg.repo_show(), None
            ));
            // Groups
            if pkg.groups() != "" {
                imp.model.append(&PropObject::new(
                    "Groups", &pkg.groups(), None
                ));
            }
            // Provides
            if !pkg.provides().is_empty() {
                imp.model.append(&PropObject::new(
                    "Provides", &self.propvec_to_wrapstring(&pkg.provides()), None
                ));
            }
            // Depends
            imp.model.append(&PropObject::new(
                "Dependencies ", &self.propvec_to_linkstring(&pkg.depends()), None
            ));
            // Optdepends
            if !pkg.optdepends().is_empty() {
                imp.model.append(&PropObject::new(
                    "Optional", &self.propvec_to_linkstring(&pkg.optdepends()), None
                ));
            }
            // Required by
            imp.model.append(&PropObject::new(
                "Required by", &self.propvec_to_linkstring(&required_by), None
            ));
            // Optional for
            if !optional_for.is_empty() {
                imp.model.append(&PropObject::new(
                    "Optional For", &self.propvec_to_linkstring(&optional_for), None
                ));
            }
            // Conflicts
            if !pkg.conflicts().is_empty() {
                imp.model.append(&PropObject::new(
                    "Conflicts With", &self.propvec_to_linkstring(&pkg.conflicts()), None
                ));
            }
            // Replaces
            if !pkg.replaces().is_empty() {
                imp.model.append(&PropObject::new(
                    "Replaces", &self.propvec_to_linkstring(&pkg.replaces()), None
                ));
            }
            // Architecture
            if pkg.architecture() != "" {
                imp.model.append(&PropObject::new(
                    "Architecture", &pkg.architecture(), None
                ));
            }
            // Packager
            if pkg.packager() != "" {
                imp.model.append(&PropObject::new(
                    "Packager", &self.prop_to_packager(&pkg.packager()), None
                ));
            }
            // Build date
            imp.model.append(&PropObject::new(
                "Build Date", &pkg.build_date_long(), None
            ));
            // Install date
            if pkg.install_date() != 0 {
                imp.model.append(&PropObject::new(
                    "Install Date", &pkg.install_date_long(), None
                ));
            }
            // Download size
            if pkg.download_size() != 0 {
                imp.model.append(&PropObject::new(
                    "Download Size", &pkg.download_size_string(), None
                ));
            }
            // Installed size
            imp.model.append(&PropObject::new(
                "Installed Size", &pkg.install_size_string(), None
            ));
            // Has script
            imp.model.append(&PropObject::new(
                "Install Script", if pkg.has_script() {"Yes"} else {"No"}, None
            ));
            // SHA256 sum
            if pkg.sha256sum() != "" {
                imp.model.append(&PropObject::new(
                    "SHA256 Sum", &pkg.sha256sum(), None
                ));
            }
            // MD5 sum
            if pkg.md5sum() != "" {
                imp.model.append(&PropObject::new(
                    "MD5 Sum", &pkg.md5sum(), None
                ));
            }
        }

        imp.empty_label.set_visible(!pkg.is_some());
    }

    pub fn display_prev(&self) {
        let hist_sel = self.history_selection();

        let hist_index = hist_sel.selected();

        if hist_index > 0 {
            hist_sel.set_selected(hist_index - 1);

            if let Some(pkg) = hist_sel.selected_item().and_downcast::<PkgObject>() {
                self.display_package(Some(&pkg));
            }
        }
    }

    pub fn display_next(&self) {
        let hist_sel = self.history_selection();

        let hist_index = hist_sel.selected();

        if hist_sel.n_items() > 0 && hist_index < hist_sel.n_items() - 1 {
            hist_sel.set_selected(hist_index + 1);

            if let Some(pkg) = hist_sel.selected_item().and_downcast::<PkgObject>() {
                self.display_package(Some(&pkg));
            }
        }
    }

    //-----------------------------------
    // Public display helper functions
    //-----------------------------------
    pub fn prop_to_esc_string(&self, prop: &str) -> String {
        glib::markup_escape_text(prop).to_string()
    }

    pub fn prop_to_esc_url(&self, prop: &str) -> String {
        format!("<a href=\"{url}\">{url}</a>", url=glib::markup_escape_text(prop).to_string())
    }

    pub fn prop_to_packager(&self, prop: &str) -> String {
        lazy_static! {
            static ref EXPR: Regex = Regex::new("^([^<]+)<([^>]+)>$").unwrap();
        }

        EXPR.replace_all(&prop, "$1&lt;<a href='mailto:$2'>$2</a>&gt;").to_string()
    }

    pub fn propvec_to_wrapstring(&self, prop_vec: &Vec<String>) -> String {
        glib::markup_escape_text(&prop_vec.join("   ")).to_string()
    }

    pub fn propvec_to_linkstring(&self, prop_vec: &Vec<String>) -> String {
        if prop_vec.is_empty() {
            String::from("None")
        } else {
            lazy_static! {
                static ref EXPR: Regex = Regex::new("(^|   |   \n)([a-zA-Z0-9@._+-]+)(?=&gt;|&lt;|<|>|=|:|   |\n|$)").unwrap();
            }

            let prop_str = self.propvec_to_wrapstring(prop_vec);

            EXPR.replace_all(&prop_str, "$1<a href='pkg://$2'>$2</a>").to_string()
        }
    }
}
