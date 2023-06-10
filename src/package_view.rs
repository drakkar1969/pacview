use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;

use glib::subclass::Signal;
use glib::once_cell::sync::Lazy;

use crate::pkg_object::PkgObject;

//------------------------------------------------------------------------------
// MODULE: PackageView
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/package_view.ui")]
    pub struct PackageView {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub click_gesture: TemplateChild<gtk::GestureClick>,
        #[template_child]
        pub popover_menu: TemplateChild<gtk::PopoverMenu>,
        #[template_child]
        pub selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub repo_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub status_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub search_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub filter_model: TemplateChild<gtk::FilterListModel>,
        #[template_child]
        pub model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub empty_label: TemplateChild<gtk::Label>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PackageView {
        const NAME: &'static str = "PackageView";
        type Type = super::PackageView;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PackageView {
        //-----------------------------------
        // Custom signals
        //-----------------------------------
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder("selected")
                        .param_types([Option::<PkgObject>::static_type()])
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }

        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            // Bind item count to empty label visibility
            self.filter_model.bind_property("n-items", &self.empty_label.get(), "visible")
                .transform_to(|_, n_items: u32| Some(n_items == 0))
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
        }
    }

    impl WidgetImpl for PackageView {}
    impl BinImpl for PackageView {}

    #[gtk::template_callbacks]
    impl PackageView {
        //-----------------------------------
        // Signal handlers
        //-----------------------------------
        #[template_callback]
        fn on_selected(&self) {
            let selected_item = self.selection.selected_item()
                .and_downcast::<PkgObject>();

            self.obj().emit_by_name::<()>("selected", &[&selected_item]);
        }

        #[template_callback]
        fn on_clicked(&self, _n_press: i32, x: f64, y: f64) {
            let button = self.click_gesture.current_button();

            if button == gdk::BUTTON_SECONDARY {
                let rect = gdk::Rectangle::new(x as i32, y as i32, 0, 0);

                self.popover_menu.set_pointing_to(Some(&rect));
                self.popover_menu.popup();
            }
        }
    }
}

//------------------------------------------------------------------------------
// PUBLIC IMPLEMENTATION: PackageView
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PackageView(ObjectSubclass<imp::PackageView>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PackageView {
    //-----------------------------------
    // Public new function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
