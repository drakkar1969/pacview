use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;

use glib::subclass::Signal;
use glib::once_cell::sync::Lazy;
use glib::clone;

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
        pub popover_menu: TemplateChild<gtk::PopoverMenu>,
        #[template_child]
        pub view: TemplateChild<gtk::ColumnView>,
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

            let obj = self.obj();

            obj.setup_widgets();
            obj.setup_controllers();
            obj.setup_signals();
        }
    }

    impl WidgetImpl for PackageView {}
    impl BinImpl for PackageView {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PackageView
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PackageView(ObjectSubclass<imp::PackageView>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PackageView {
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
        let imp = self.imp();

        // Bind item count to empty label visibility
        imp.filter_model.bind_property("n-items", &imp.empty_label.get(), "visible")
            .transform_to(|_, n_items: u32| Some(n_items == 0))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
    }

    //-----------------------------------
    // Setup controllers
    //-----------------------------------
    fn setup_controllers(&self) {
        let imp = self.imp();

        // Column view click gesture
        let gesture = gtk::GestureClick::new();

        gesture.set_button(0);

        gesture.connect_pressed(clone!(@weak imp => move |gesture, _, x, y| {
            let button = gesture.current_button();

            if button == gdk::BUTTON_SECONDARY {
                let rect = gdk::Rectangle::new(x as i32, y as i32, 0, 0);

                imp.popover_menu.set_pointing_to(Some(&rect));
                imp.popover_menu.popup();
            }
        }));

        self.add_controller(gesture);
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Column view selected item property notify signal
        imp.selection.connect_selected_item_notify(clone!(@weak self as obj => move |selection| {
            let selected_item = selection.selected_item()
                .and_downcast::<PkgObject>();

            obj.emit_by_name::<()>("selected", &[&selected_item]);
        }));
    }
    //-----------------------------------
    // Sort columns helper function
    //-----------------------------------
    fn sort_columns(&self, column_ids: &glib::StrV) {
        let columns = self.imp().view.columns();

        // Iterate through column IDs
        for (i, id) in column_ids.iter().enumerate() {
            // If column exists with given ID, insert it at position
            if let Some(col) = columns.iter::<gtk::ColumnViewColumn>().flatten()
                .find(|col| col.id().unwrap() == *id)
            {
                self.imp().view.insert_column(i as u32, &col);
            }
        }

        // Show/hide columns
        for col in columns.iter::<gtk::ColumnViewColumn>().flatten() {
            col.set_visible(column_ids.contains(&col.id().unwrap()));
        }
    }

    //-----------------------------------
    // Public column functions
    //-----------------------------------
    pub fn reset_columns(&self) {
        self.sort_columns(&["package", "version", "repository", "status", "date", "size"].into());
    }

    pub fn set_columns(&self, column_ids: &glib::StrV) {
        self.sort_columns(column_ids);
    }

    pub fn columns(&self) -> glib::StrV {
        // Get visible column IDs
        self.imp().view.columns()
            .iter::<gtk::ColumnViewColumn>()
            .flatten()
            .filter_map(|col| if col.is_visible() {Some(col.id().unwrap())} else {None})
            .collect::<Vec<glib::GString>>()
            .into()
    }

    pub fn set_sorting(&self, id: &glib::GString, ascending: bool) {
        // Find sort column by ID
        let col = self.imp().view.columns().iter::<gtk::ColumnViewColumn>()
            .flatten()
            .find(|col| col.id().unwrap() == *id);

        // Set sort column/order
        self.imp().view.sort_by_column(col.as_ref(), if ascending {gtk::SortType::Ascending} else {gtk::SortType::Descending});
    }

    pub fn sorting(&self) -> (glib::GString, bool) {
        // Get view sorter
        let sorter = self.imp().view.sorter()
            .and_downcast::<gtk::ColumnViewSorter>()
            .expect("Must be a 'ColumnViewSorter'");

        // Get sort column ID
        let sort_col = sorter.primary_sort_column().map_or(
            glib::GString::from(""),
            |col| col.id().unwrap()
        );

        // Get sort order
        let sort_asc = sorter.primary_sort_order() == gtk::SortType::Ascending;

        (sort_col, sort_asc)
    }
}