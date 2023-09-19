use std::cell::RefCell;
use std::collections::HashMap;

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::closure_local;

use url::Url;

use crate::text_layout::{TextLayout, PropType};
use crate::property_label::PropertyLabel;
use crate::property_value::PropertyValue;
use crate::pkg_object::{PkgObject, PkgFlags};

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
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub grid: TemplateChild<gtk::Grid>,

        #[template_child]
        pub overlay_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub overlay_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub overlay_prev_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub overlay_next_button: TemplateChild<gtk::Button>,

        #[property(get, set)]
        pkg_model: RefCell<Option<gio::ListStore>>,

        #[property(name = "pkg", type = Option<PkgObject>, get = Self::pkg, set = Self::set_pkg, nullable)]
        pub history_selection: RefCell<gtk::SingleSelection>,

        pub property_map: RefCell<HashMap<String, (PropertyLabel, PropertyValue)>>,
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
            TextLayout::ensure_type();

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
        }
    }

    impl WidgetImpl for InfoPane {}
    impl BinImpl for InfoPane {}

    impl InfoPane {
        //-----------------------------------
        // Custom pkg property getter/setter
        //-----------------------------------
        fn pkg(&self) -> Option<PkgObject> {
            self.history_selection.borrow().selected_item()
                .and_downcast::<PkgObject>()
        }

        fn set_pkg(&self, pkg: Option<&PkgObject>) {
            let hist_model = self.history_selection.borrow().model()
                .and_downcast::<gio::ListStore>()
                .expect("Must be a 'ListStore'");

            hist_model.remove_all();

            if let Some(pkg) = pkg {
                hist_model.append(pkg);
            }

            self.obj().update_display();
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
    // Value label link handler
    //-----------------------------------
    fn link_handler(&self, link: &str) -> bool {
        if let Some(url) = Url::parse(&link).ok().filter(|url| url.scheme() == "pkg") {
            if let Some(pkg_name) = url.domain() {
                let pkg_model = self.pkg_model().unwrap();

                // Find link package by name
                let mut new_pkg = pkg_model.iter::<PkgObject>()
                    .flatten()
                    .find(|pkg| pkg.name() == pkg_name);

                // If link package is none, find by provides
                if new_pkg.is_none() {
                    new_pkg = pkg_model.iter::<PkgObject>()
                        .flatten()
                        .find(|pkg| {
                            pkg.provides().iter().any(|s| s.contains(pkg_name))
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
                    self.update_display();
                }
            }

            // Link handled
            return true
        }

        // Link not handled
        false
    }

    //-----------------------------------
    // Add property function
    //-----------------------------------
    fn add_property(&self, row: i32, ptype: PropType, label: &str) {
        let imp = self.imp();

        let property_label = PropertyLabel::new(label);

        imp.grid.attach(&property_label, 0, row, 1, 1);

        let property_value = PropertyValue::new(ptype, closure_local!(@watch self as obj => move |_: TextLayout, link: String| -> bool {
            obj.link_handler(&link)
        }));

        imp.grid.attach(&property_value, 1, row, 1, 1);

        imp.property_map.borrow_mut().insert(label.to_string(), (property_label, property_value));
    }

    //-----------------------------------
    // Set property value function
    //-----------------------------------
    fn set_property_value(&self, label: &str, visible: bool, value: &str, icon: Option<&str>) {
        if let Some((property_label, property_value)) = self.imp().property_map.borrow().get(label) {
            property_label.set_visible(visible);
            property_value.set_visible(visible);

            if visible {
                property_value.set_icon(icon);
                property_value.set_text(value);
            }
        }
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Initialize history selection
        let history_model = gio::ListStore::new::<PkgObject>();

        imp.history_selection.replace(gtk::SingleSelection::new(Some(history_model)));

        // Add property rows
        self.add_property(0, PropType::Title, "Name");
        self.add_property(1, PropType::Text, "Version");
        self.add_property(2, PropType::Text, "Description");
        self.add_property(3, PropType::Link, "Package URL");
        self.add_property(4, PropType::Link, "URL");
        self.add_property(5, PropType::Text, "Licenses");
        self.add_property(6, PropType::Text, "Status");
        self.add_property(7, PropType::Text, "Repository");
        self.add_property(8, PropType::Text, "Groups");
        self.add_property(9, PropType::Text, "Provides");
        self.add_property(10, PropType::LinkList, "Dependencies ");
        self.add_property(11, PropType::LinkList, "Optional");
        self.add_property(12, PropType::LinkList, "Required By");
        self.add_property(13, PropType::LinkList, "Optional For");
        self.add_property(14, PropType::LinkList, "Conflicts With");
        self.add_property(15, PropType::LinkList, "Replaces");
        self.add_property(16, PropType::Text, "Architecture");
        self.add_property(17, PropType::Packager, "Packager");
        self.add_property(18, PropType::Text, "Build Date");
        self.add_property(19, PropType::Text, "Install Date");
        self.add_property(20, PropType::Text, "Download Size");
        self.add_property(21, PropType::Text, "Installed Size");
        self.add_property(22, PropType::Text, "Install Script");
        self.add_property(23, PropType::Text, "SHA256 Sum");
        self.add_property(24, PropType::Text, "MD5 Sum");
    }

    //-----------------------------------
    // Public display functions
    //-----------------------------------
    pub fn update_display(&self) {
        let imp = self.imp();

        // Set stack visible page
        imp.stack.set_visible_child_name(if self.pkg().is_some() {"properties"} else {"empty"});

        // If package is not none, display it
        if let Some(pkg) = self.pkg() {
            let hist_sel = imp.history_selection.borrow();

            let overlay_visible = hist_sel.n_items() > 1;

            // Set infopane toolbar previous/next box visibility
            if imp.overlay_box.is_visible() != overlay_visible {
                imp.overlay_box.set_visible(overlay_visible);
            }

            // Set infopane toolbar label
            if overlay_visible {
                imp.overlay_label.set_label(&format!("{}/{}", hist_sel.selected() + 1, hist_sel.n_items()));
            }

            // Set infopane toolbar prev/next button states
            let prev_sensitive = hist_sel.selected() > 0;
            
            if imp.overlay_prev_button.is_sensitive() != prev_sensitive {
                imp.overlay_prev_button.set_sensitive(prev_sensitive);
            }

            let next_sensitive = hist_sel.n_items() > 0 && hist_sel.selected() < hist_sel.n_items() - 1;

            if imp.overlay_next_button.is_sensitive() != next_sensitive {
                imp.overlay_next_button.set_sensitive(next_sensitive);
            }

            // Name
            self.set_property_value("Name", true, &pkg.name(), None);
            // Version
            self.set_property_value("Version", true, &pkg.version(), if pkg.has_update() {Some("pkg-update")} else {None});
            // Description
            self.set_property_value("Description", true, &pkg.description(), None);
            // Package URL
            self.set_property_value("Package URL", true, &self.prop_to_package_url(&pkg), None);
            // URL
            self.set_property_value("URL", pkg.url() != "", &pkg.url(), None);
            // Licenses
            self.set_property_value("Licenses", pkg.licenses() != "", &pkg.licenses(), None);
            // Status
            let status = &pkg.status();
            let status_icon = pkg.status_icon();
            self.set_property_value("Status", true, if pkg.flags().intersects(PkgFlags::INSTALLED) {status} else {"not installed"}, if pkg.flags().intersects(PkgFlags::INSTALLED) {Some(&status_icon)} else {None});
            // Repository
            self.set_property_value("Repository", true, &pkg.repo_show(), None);
            // Groups
            self.set_property_value("Groups", pkg.groups() != "", &pkg.groups(), None);
            // Provides
            self.set_property_value("Provides", !pkg.provides().is_empty(), &pkg.provides().join("     "), None);
            // Depends
            self.set_property_value("Dependencies ", true, &pkg.depends().join("     "), None);
            // Optdepends
            self.set_property_value("Optional", !pkg.optdepends().is_empty(), &pkg.optdepends().join("     "), None);
            // Required by
            self.set_property_value("Required By", true, &pkg.required_by().join("     "), None);
            // Optional for
            let optional_for = pkg.optional_for();
            self.set_property_value("Optional For", !optional_for.is_empty(), &optional_for.join("     "), None);
            // Conflicts
            self.set_property_value("Conflicts With", !pkg.conflicts().is_empty(), &pkg.conflicts().join("     "), None);
            // Replaces
            self.set_property_value("Replaces", !pkg.replaces().is_empty(), &pkg.replaces().join("     "), None);
            // Architecture
            self.set_property_value("Architecture", pkg.architecture() != "", &pkg.architecture(), None);
            // Packager
            self.set_property_value("Packager", true, &pkg.packager(), None);
            // Build date
            self.set_property_value("Build Date", true, &pkg.build_date_long(), None);
            // Install date
            self.set_property_value("Install Date", pkg.install_date() != 0, &pkg.install_date_long(), None);
            // Download size
            self.set_property_value("Download Size", pkg.download_size() != 0, &pkg.download_size_string(), None);
            // Installed size
            self.set_property_value("Installed Size", true, &pkg.install_size_string(), None);
            // Has script
            self.set_property_value("Install Script", true, if pkg.has_script() {"Yes"} else {"No"}, None);
            // SHA256 sum
            self.set_property_value("SHA256 Sum", pkg.sha256sum() != "", &pkg.sha256sum(), None);
            // MD5 sum
            self.set_property_value("MD5 Sum", pkg.md5sum() != "", &pkg.md5sum(), None);
        }
    }

    pub fn display_prev(&self) {
        let hist_sel = self.imp().history_selection.borrow();

        if hist_sel.selected() > 0 {
            hist_sel.set_selected(hist_sel.selected() - 1);

            if hist_sel.selected_item().is_some() {
                self.update_display();
            }
        }
    }

    pub fn display_next(&self) {
        let hist_sel = self.imp().history_selection.borrow();

        if hist_sel.n_items() > 0 && hist_sel.selected() < hist_sel.n_items() - 1 {
            hist_sel.set_selected(hist_sel.selected() + 1);

            if hist_sel.selected_item().is_some() {
                self.update_display();
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
            url = format!("https://www.archlinux.org/packages/{repo}/{arch}/{name}",
                repo=pkg.repo_show(),
                arch=pkg.architecture(),
                name=pkg.name());
        } else if &pkg.repo_show() == "aur" {
            url = format!("https://aur.archlinux.org/packages/{name}", name=pkg.name());
        }

        url
    }

    //-----------------------------------
    // Public update property value function
    //-----------------------------------
    pub fn update_property_value(&self, label: &str, value: &str, icon: Option<&str>) {
        if let Some((_, property_value)) = self.imp().property_map.borrow().get(label) {
            property_value.set_icon(icon);
            property_value.set_text(value);
        }
    }
}
