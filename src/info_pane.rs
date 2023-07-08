use std::cell::RefCell;

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use crate::value_row::ValueRow;
use crate::pkg_object::{PkgObject, PkgFlags};
use crate::prop_object::{PropObject, PropType};

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
        pkg_model: RefCell<gio::ListStore>,

        pub history_selection: RefCell<gtk::SingleSelection>,
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

        // Initialize history selection
        let history_model = gio::ListStore::new(PkgObject::static_type());

        self.imp().history_selection.replace(gtk::SingleSelection::new(Some(history_model)));
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Value factory setup signal
        imp.value_factory.connect_setup(clone!(@weak self as obj => move |_, item| {
            let value_row = ValueRow::new(obj);

            item
                .downcast_ref::<gtk::ListItem>()
                .expect("Must be a 'ListItem'")
                .set_child(Some(&value_row));
        }));

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
        });
    }

    //-----------------------------------
    // Public value label link handler
    //-----------------------------------
    pub fn link_handler(&self, pkg_name: &str) {
        // Find link package by name
        let mut new_pkg = self.pkg_model().iter::<PkgObject>().flatten()
            .find(|pkg| pkg.name() == pkg_name);

        // If link package is none, find by provides
        if new_pkg.is_none() {
            new_pkg = self.pkg_model().iter::<PkgObject>().flatten().find(|pkg| {
                pkg.provides().iter().any(|s| s.contains(&pkg_name))
            });
        }

        // If link package found
        if let Some(new_pkg) = new_pkg {
            let hist_sel = self.imp().history_selection.borrow();

            let hist_model = hist_sel.model()
                .and_downcast::<gio::ListStore>()
                .expect("Must be a 'ListStore'");

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

    //-----------------------------------
    // Public display functions
    //-----------------------------------
    pub fn display_package(&self, pkg: Option<&PkgObject>) {
        let imp = self.imp();

        let hist_sel = imp.history_selection.borrow();

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

            // Name
            imp.model.append(&PropObject::new(
                "Name", &pkg.name(), None, PropType::Title
            ));
            // Version
            imp.model.append(&PropObject::new(
                "Version", &pkg.version(), if pkg.has_update() {Some("pkg-update")} else {None}, PropType::Text
            ));
            // Description
            imp.model.append(&PropObject::new(
                "Description", &pkg.description(), None, PropType::Text
            ));
            // Package URL
            imp.model.append(&PropObject::new(
                "Package URL", &self.prop_to_package_url(&pkg), None, PropType::Link
            ));
            // URL
            if pkg.url() != "" {
                imp.model.append(&PropObject::new(
                    "URL", &pkg.url(), None, PropType::Link
                ));
            }
            // Licenses
            if pkg.licenses() != "" {
                imp.model.append(&PropObject::new(
                    "Licenses", &&pkg.licenses(), None, PropType::Text
                ));
            }
            // Status
            let status = &pkg.status();
            let status_icon = pkg.status_icon();

            imp.model.append(&PropObject::new(
                "Status", if pkg.flags().intersects(PkgFlags::INSTALLED) {&status} else {"not installed"}, if pkg.flags().intersects(PkgFlags::INSTALLED) {Some(&status_icon)} else {None}, PropType::Text
            ));
            // Repository
            imp.model.append(&PropObject::new(
                "Repository", &pkg.repo_show(), None, PropType::Text
            ));
            // Groups
            if pkg.groups() != "" {
                imp.model.append(&PropObject::new(
                    "Groups", &pkg.groups(), None, PropType::Text
                ));
            }
            // Provides
            if !pkg.provides().is_empty() {
                imp.model.append(&PropObject::new(
                    "Provides", &pkg.provides().join("   "), None, PropType::Text
                ));
            }
            // Depends
            imp.model.append(&PropObject::new(
                "Dependencies ", &pkg.depends().join("   "), None, PropType::LinkList
            ));
            // Optdepends
            if !pkg.optdepends().is_empty() {
                imp.model.append(&PropObject::new(
                    "Optional", &pkg.optdepends().join("   "), None, PropType::LinkList
                ));
            }
            // Required by
            imp.model.append(&PropObject::new(
                "Required by", &pkg.required_by().join("   "), None, PropType::LinkList
            ));
            // Optional for
            let optional_for = pkg.optional_for();
            
            if !optional_for.is_empty() {
                imp.model.append(&PropObject::new(
                    "Optional For", &optional_for.join("   "), None, PropType::LinkList
                ));
            }
            // Conflicts
            if !pkg.conflicts().is_empty() {
                imp.model.append(&PropObject::new(
                    "Conflicts With", &pkg.conflicts().join("   "), None, PropType::LinkList
                ));
            }
            // Replaces
            if !pkg.replaces().is_empty() {
                imp.model.append(&PropObject::new(
                    "Replaces", &pkg.replaces().join("   "), None, PropType::LinkList
                ));
            }
            // Architecture
            if pkg.architecture() != "" {
                imp.model.append(&PropObject::new(
                    "Architecture", &pkg.architecture(), None, PropType::Text
                ));
            }
            // Packager
            if pkg.packager() != "" {
                imp.model.append(&PropObject::new(
                    "Packager", &pkg.packager(), None, PropType::Packager
                ));
            }
            // Build date
            imp.model.append(&PropObject::new(
                "Build Date", &pkg.build_date_long(), None, PropType::Text
            ));
            // Install date
            if pkg.install_date() != 0 {
                imp.model.append(&PropObject::new(
                    "Install Date", &pkg.install_date_long(), None, PropType::Text
                ));
            }
            // Download size
            if pkg.download_size() != 0 {
                imp.model.append(&PropObject::new(
                    "Download Size", &pkg.download_size_string(), None, PropType::Text
                ));
            }
            // Installed size
            imp.model.append(&PropObject::new(
                "Installed Size", &pkg.install_size_string(), None, PropType::Text
            ));
            // Has script
            imp.model.append(&PropObject::new(
                "Install Script", if pkg.has_script() {"Yes"} else {"No"}, None, PropType::Text
            ));
            // SHA256 sum
            if pkg.sha256sum() != "" {
                imp.model.append(&PropObject::new(
                    "SHA256 Sum", &pkg.sha256sum(), None, PropType::Text
                ));
            }
            // MD5 sum
            if pkg.md5sum() != "" {
                imp.model.append(&PropObject::new(
                    "MD5 Sum", &pkg.md5sum(), None, PropType::Text
                ));
            }
        }

        imp.empty_label.set_visible(!pkg.is_some());
    }

    pub fn display_prev(&self) {
        let hist_sel = self.imp().history_selection.borrow();

        let hist_index = hist_sel.selected();

        if hist_index > 0 {
            hist_sel.set_selected(hist_index - 1);

            if let Some(pkg) = hist_sel.selected_item().and_downcast::<PkgObject>() {
                self.display_package(Some(&pkg));
            }
        }
    }

    pub fn display_next(&self) {
        let hist_sel = self.imp().history_selection.borrow();

        let hist_index = hist_sel.selected();

        if hist_sel.n_items() > 0 && hist_index < hist_sel.n_items() - 1 {
            hist_sel.set_selected(hist_index + 1);

            if let Some(pkg) = hist_sel.selected_item().and_downcast::<PkgObject>() {
                self.display_package(Some(&pkg));
            }
        }
    }

    //-----------------------------------
    // Public display helper function
    //-----------------------------------
    pub fn prop_to_package_url(&self, pkg: &PkgObject) -> String {
        let mut url = String::from("");

        let default_repos = ["core", "extra", "multilib"];

        if default_repos.contains(&pkg.repo_show().as_str()) {
            url = format!("https://www.archlinux.org/packages/{repo}/{arch}/{name}", repo=pkg.repo_show(), arch=pkg.architecture(), name=pkg.name());
        } else if &pkg.repo_show() == "aur" {
            url = format!("https://aur.archlinux.org/packages/{name}", name=pkg.name());
        }

        url
    }

    //-----------------------------------
    // Public get/set pkg functions
    //-----------------------------------
    pub fn pkg(&self) -> Option<PkgObject> {
        self.imp().history_selection.borrow().selected_item()
            .and_downcast::<PkgObject>()
    }

    pub fn set_pkg(&self, pkg: Option<&PkgObject>) {
        let hist_model = self.imp().history_selection.borrow().model()
            .and_downcast::<gio::ListStore>()
            .expect("Must be a 'ListStore'");

        hist_model.remove_all();

        self.display_package(pkg);

        if let Some(pkg) = pkg {
            hist_model.append(pkg);
        }
    }

    //-----------------------------------
    // Public update property function
    //-----------------------------------
    pub fn update_prop(&self, label: &str, value: &str, icon: Option<&str>) {
        if let Some(prop) = self.imp().model.iter::<PropObject>().flatten()
            .find(|prop| prop.label() == label)
        {
            prop.set_value(value);

            if let Some(icon) = icon {
                prop.set_icon(icon);
            }

        }
    }
}
