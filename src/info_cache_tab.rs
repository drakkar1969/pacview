use std::cell::RefCell;
use std::fmt::Write as _;

use gtk::subclass::prelude::*;
use gtk::prelude::*;
use gtk::{glib, gio};
use glib::clone;

use crate::pkg_object::PkgObject;
use crate::utils::AppInfoExt;

//------------------------------------------------------------------------------
// MODULE: InfoCacheTab
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::InfoCacheTab)]
    #[template(resource = "/com/github/PacView/ui/info_cache_tab.ui")]
    pub struct InfoCacheTab {
        #[template_child]
        pub(super) header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) count_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) paused_status: TemplateChild<adw::StatusPage>,

        #[property(get, set)]
        pkg_name: RefCell<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for InfoCacheTab {
        const NAME: &'static str = "InfoCacheTab";
        type Type = super::InfoCacheTab;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for InfoCacheTab {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
        }
    }
    impl WidgetImpl for InfoCacheTab {}
    impl BoxImpl for InfoCacheTab {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: InfoCacheTab
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct InfoCacheTab(ObjectSubclass<imp::InfoCacheTab>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl InfoCacheTab {
    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Open button clicked signal
        imp.open_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                let cache_file = imp.selection.selected_item()
                    .and_downcast::<gtk::StringObject>()
                    .expect("Failed to downcast to 'StringObject'")
                    .string();

                glib::spawn_future_local(async move {
                    AppInfoExt::open_containing_folder(&cache_file).await;
                });
            }
        ));

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = tab)] self,
            move |_| {
                let mut output = String::new();

                let _ = writeln!(output, "## {}\n|Cache Files|\n|---|", tab.pkg_name());

                for obj in tab.imp().model.iter::<gtk::StringObject>()
                    .flatten() {
                        let _ = writeln!(output, "{}", obj.string());
                    }

                tab.clipboard().set_text(&output);
            }
        ));

        // View activate signal
        imp.view.connect_activate(clone!(
            #[weak] imp,
            move |_, _| {
                if imp.open_button.is_sensitive() {
                    imp.open_button.emit_clicked();
                }
            }
        ));

        // Selection items changed signal
        imp.selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                imp.count_label.set_label(&n_items.to_string());
                imp.open_button.set_sensitive(n_items > 0);
                imp.copy_button.set_sensitive(n_items > 0);
            }
        ));
    }

    //---------------------------------------
    // Pause view function
    //---------------------------------------
    pub fn pause_view(&self) {
        let imp = self.imp();

        imp.paused_status.set_visible(true);
        imp.model.remove_all();
    }

    //---------------------------------------
    // Update view function
    //---------------------------------------
    pub fn update_view(&self, pkg: &PkgObject) {
        let imp = self.imp();

        imp.paused_status.set_visible(false);

        // Populate view
        glib::spawn_future_local(clone!(
            #[weak] imp,
            #[weak] pkg,
            async move {
                let cache_list: Vec<gtk::StringObject> = pkg.cache_future().await.iter()
                    .map(|cache_file| gtk::StringObject::new(cache_file))
                    .collect();

                imp.model.splice(0, imp.model.n_items(), &cache_list);
            }
        ));

        self.set_pkg_name(pkg.name());
    }
}
