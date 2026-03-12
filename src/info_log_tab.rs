use std::cell::RefCell;
use std::fmt::Write as _;

use gtk::subclass::prelude::*;
use gtk::prelude::*;
use gtk::{glib, gio};
use glib::clone;

use crate::{
    pkg_object::PkgObject,
    utils::AppInfoExt
};

//------------------------------------------------------------------------------
// MODULE: InfoLogTab
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::InfoLogTab)]
    #[template(resource = "/com/github/PacView/ui/info_log_tab.ui")]
    pub struct InfoLogTab {
        #[template_child]
        pub(super) log_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) log_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) log_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) log_selection: TemplateChild<gtk::NoSelection>,
        #[template_child]
        pub(super) log_spinner: TemplateChild<adw::Spinner>,

        #[template_child]
        pub(super) cache_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) cache_count_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) cache_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) cache_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) cache_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) cache_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) cache_selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) cache_spinner: TemplateChild<adw::Spinner>,

        #[property(get, set)]
        pkg_name: RefCell<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for InfoLogTab {
        const NAME: &'static str = "InfoLogTab";
        type Type = super::InfoLogTab;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for InfoLogTab {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
        }
    }
    impl WidgetImpl for InfoLogTab {}
    impl BoxImpl for InfoLogTab {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: InfoLogTab
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct InfoLogTab(ObjectSubclass<imp::InfoLogTab>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl InfoLogTab {
    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Log copy button clicked signal
        imp.log_copy_button.connect_clicked(clone!(
            #[weak(rename_to = tab)] self,
            move |_| {
                let mut output = String::new();

                let _ = writeln!(output, "## {}\n|Log Messages|\n|---|", tab.pkg_name());

                for obj in tab.imp().log_model.iter::<gtk::StringObject>()
                    .flatten() {
                        let _ = writeln!(output, "{}", obj.string());
                    }

                tab.clipboard().set_text(&output);
            }
        ));

        // Log selection items changed signal
        imp.log_selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                imp.log_copy_button.set_sensitive(n_items > 0);
            }
        ));

        // Cache open button clicked signal
        imp.cache_open_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                let cache_file = imp.cache_selection.selected_item()
                    .and_downcast::<gtk::StringObject>()
                    .expect("Failed to downcast to 'StringObject'")
                    .string();

                glib::spawn_future_local(async move {
                    AppInfoExt::open_containing_folder(&cache_file).await;
                });
            }
        ));

        // Cache copy button clicked signal
        imp.cache_copy_button.connect_clicked(clone!(
            #[weak(rename_to = tab)] self,
            move |_| {
                let mut output = String::new();

                let _ = writeln!(output, "## {}\n|Cache Files|\n|---|", tab.pkg_name());

                for obj in tab.imp().cache_model.iter::<gtk::StringObject>()
                    .flatten() {
                        let _ = writeln!(output, "{}", obj.string());
                    }

                tab.clipboard().set_text(&output);
            }
        ));

        // Cache view activate signal
        imp.cache_view.connect_activate(clone!(
            #[weak] imp,
            move |_, _| {
                if imp.cache_open_button.is_sensitive() {
                    imp.cache_open_button.emit_clicked();
                }
            }
        ));

        // Cache selection items changed signal
        imp.cache_selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                imp.cache_count_label.set_label(&n_items.to_string());
                imp.cache_open_button.set_sensitive(n_items > 0);
                imp.cache_copy_button.set_sensitive(n_items > 0);
            }
        ));
    }

    //---------------------------------------
    // Pause views function
    //---------------------------------------
    pub fn pause_views(&self) {
        let imp = self.imp();

        imp.log_spinner.set_visible(true);
        imp.log_model.remove_all();

        imp.cache_spinner.set_visible(true);
        imp.cache_model.remove_all();
    }

    //---------------------------------------
    // Update views function
    //---------------------------------------
    pub fn update_views(&self, pkg: &PkgObject) {
        let imp = self.imp();

        imp.log_spinner.set_visible(false);
        imp.cache_spinner.set_visible(false);

        // Populate log view
        glib::spawn_future_local(clone!(
            #[weak] imp,
            #[weak] pkg,
            async move {
                let log_lines: Vec<gtk::StringObject> = pkg.log_future().await.iter()
                    .map(|line| gtk::StringObject::new(line))
                    .collect();

                imp.log_model.splice(0, imp.log_model.n_items(), &log_lines);
            }
        ));

        // Populate cache view
        glib::spawn_future_local(clone!(
            #[weak] imp,
            #[weak] pkg,
            async move {
                let cache_list: Vec<gtk::StringObject> = pkg.cache_future().await.iter()
                    .map(|cache_file| gtk::StringObject::new(cache_file))
                    .collect();

                imp.cache_model.splice(0, imp.cache_model.n_items(), &cache_list);
            }
        ));

        self.set_pkg_name(pkg.name());
    }
}
