use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use std::cmp::Ordering;
use std::borrow::Cow;

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::subclass::Signal;
use glib::clone;

use tokio::sync::Mutex as TokioMutex;
use tokio_util::sync::CancellationToken;
use raur::Raur;
use raur::ArcPackage;
use futures::future;
use strum::EnumString;

use crate::window::INSTALLED_PKG_NAMES;
use crate::pkg_data::{PkgFlags, PkgData};
use crate::pkg_object::PkgObject;
use crate::search_bar::{SearchBar, SearchMode, SearchProp};
use crate::utils::Tokio;
use crate::enum_traits::EnumExt;

//------------------------------------------------------------------------------
// GLOBAL VARIABLES
//------------------------------------------------------------------------------
thread_local! {
    pub static AUR_PKGS: RefCell<Vec<PkgObject>> = const { RefCell::new(vec![]) };
}

//------------------------------------------------------------------------------
// ENUM: PackageViewStatus
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PackageViewStatus {
    #[default]
    Normal,
    PackageLoad,
    AURDownload,
}

//------------------------------------------------------------------------------
// ENUM: SortProp
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum, EnumString)]
#[strum(serialize_all = "kebab-case")]
#[repr(u32)]
#[enum_type(name = "SortProp")]
pub enum SortProp {
    #[default]
    Name,
    Version,
    Repository,
    Status,
    InstallDate,
    InstalledSize,
    Groups,
}

impl EnumExt for SortProp {}

//------------------------------------------------------------------------------
// MODULE: PackageView
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PackageView)]
    #[template(resource = "/com/github/PacView/ui/package_view.ui")]
    pub struct PackageView {
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) loading_status: TemplateChild<adw::StatusPage>,

        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) filter_model: TemplateChild<gtk::FilterListModel>,
        #[template_child]
        pub(super) pkg_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) aur_model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) repo_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) status_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub(super) search_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub(super) factory: TemplateChild<gtk::BuilderListItemFactory>,
        #[template_child]
        pub(super) sorter: TemplateChild<gtk::CustomSorter>,

        #[template_child]
        pub(super) empty_status: TemplateChild<adw::StatusPage>,

        #[property(get, set)]
        n_items: Cell<u32>,
        #[property(get, set, builder(SortProp::default()))]
        sort_prop: Cell<SortProp>,
        #[property(get, set, default = true, construct)]
        sort_ascending: Cell<bool>,

        pub(super) aur_cache: Arc<TokioMutex<HashSet<ArcPackage>>>,
        pub(super) search_cancel_token: RefCell<Option<CancellationToken>>
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
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

    #[glib::derived_properties]
    impl ObjectImpl for PackageView {
        //---------------------------------------
        // Custom signals
        //---------------------------------------
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("selected")
                        .param_types([Option::<PkgObject>::static_type()])
                        .build(),
                    Signal::builder("activated")
                        .param_types([Option::<PkgObject>::static_type()])
                        .build(),
                ]
            })
        }

        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
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
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind item count to n_items property
        imp.selection.bind_property("n-items", self, "n-items")
            .sync_create()
            .build();

        // Bind item count to empty label visibility
        imp.selection.bind_property("n-items", &imp.empty_status.get(), "visible")
            .transform_to(|_, n_items: u32| Some(n_items == 0))
            .sync_create()
            .build();

        // Set list view sorter function
        imp.sorter.set_sort_func(clone!(
            #[weak(rename_to = view)] self,
            #[upgrade_or] gtk::Ordering::Equal,
            move |item_a, item_b| {
                let pkg_a: &PkgObject = item_a
                    .downcast_ref::<PkgObject>()
                    .expect("Failed to downcast to 'PkgObject'");

                let pkg_b: &PkgObject = item_b
                    .downcast_ref::<PkgObject>()
                    .expect("Failed to downcast to 'PkgObject'");

                let sort = match view.sort_prop() {
                    SortProp::Name => { pkg_a.name().partial_cmp(&pkg_b.name()) },
                    SortProp::Version => { pkg_a.version().partial_cmp(&pkg_b.version()) },
                    SortProp::Repository => { pkg_a.repository().partial_cmp(&pkg_b.repository()) },
                    SortProp::Status => { pkg_a.status().partial_cmp(&pkg_b.status()) },
                    SortProp::InstallDate => { pkg_a.install_date().partial_cmp(&pkg_b.install_date()) },
                    SortProp::InstalledSize => { pkg_a.install_size().partial_cmp(&pkg_b.install_size()) },
                    SortProp::Groups => { pkg_a.groups().partial_cmp(&pkg_b.groups()) },
                }.unwrap_or(Ordering::Equal);

                if view.sort_ascending() {
                    sort
                } else {
                    sort.reverse()
                }.into()
            }
        ));
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // List view selected item property notify signal
        imp.selection.connect_selected_item_notify(clone!(
            #[weak(rename_to = view)] self,
            move |selection| {
                let selected_item = selection.selected_item()
                    .and_downcast::<PkgObject>();

                view.emit_by_name::<()>("selected", &[&selected_item]);
            }
        ));

        // List view activate signal
        imp.view.connect_activate(clone!(
            #[weak(rename_to = view)] self,
            move |_, index| {
                let item = view.imp().selection.item(index)
                    .and_downcast::<PkgObject>();

                view.emit_by_name::<()>("activated", &[&item]);
            }
        ));

        // Sort prop property notify signal
        self.connect_sort_prop_notify(|view| {
            view.imp().sorter.changed(gtk::SorterChange::Different);
        });

        // Sort ascending property notify signal
        self.connect_sort_ascending_notify(|view| {
            view.imp().sorter.changed(gtk::SorterChange::Inverted);
        });
    }

    //---------------------------------------
    // Public filter functions
    //---------------------------------------
    pub fn set_repo_filter(&self, repo_id: Option<&str>) {
        self.imp().repo_filter.set_search(repo_id);
    }

    pub fn set_status_filter(&self, status_id: PkgFlags) {
        self.imp().status_filter.set_filter_func(move |item| {
            let pkg: &PkgObject = item
                .downcast_ref::<PkgObject>()
                .expect("Failed to downcast to 'PkgObject'");

            pkg.flags().intersects(status_id)
        });
    }

    fn get_search_props(prop: SearchProp, pkg: &PkgObject) -> Cow<'_, [String]> {
        match prop {
            SearchProp::Name => { vec![pkg.name()].into() },
            SearchProp::NameDesc => { vec![pkg.name(), pkg.description().to_owned()].into() },
            SearchProp::Groups => { vec![pkg.groups()].into() },
            SearchProp::Deps => { Cow::Borrowed(pkg.depends()) },
            SearchProp::Optdeps => { Cow::Borrowed(pkg.optdepends()) },
            SearchProp::Provides => { Cow::Borrowed(pkg.provides()) },
            SearchProp::Files => { Cow::Borrowed(pkg.files()) },
        }
    }

    pub fn set_search_filter(&self, search_term: &str, mode: SearchMode, prop: SearchProp) {
        let imp = self.imp();

        if search_term.is_empty() {
            imp.search_filter.unset_filter_func();
        } else {
            let term = search_term.to_lowercase();

            imp.search_filter.set_filter_func(move |item| {
                let pkg: &PkgObject = item
                    .downcast_ref::<PkgObject>()
                    .expect("Failed to downcast to 'PkgObject'");

                let search_props = Self::get_search_props(prop, pkg);

                if mode == SearchMode::Exact {
                    search_props.iter().any(|s| s.eq(&term))
                } else {
                    let mut results = term.split_whitespace()
                        .map(|t| search_props.iter().any(|s| s.to_lowercase().contains(t)));

                    if mode == SearchMode::All {
                        results.all(|x| x)
                    } else {
                        results.any(|x| x)
                    }
                }
            });
        }
    }

    //---------------------------------------
    // Do search async helper function
    //---------------------------------------
    async fn do_search_async(term: &str, prop: SearchProp, installed_pkg_names: &HashSet<String>, aur_cache: &Arc<TokioMutex<HashSet<raur::ArcPackage>>>) -> Result<Vec<raur::ArcPackage>, raur::Error> {
        let handle = raur::Handle::new();

        // Return if query arg too small
        if term.len() < 2 {
            return Err(raur::Error::Aur(String::from("Query arg too small.")))
        }

        // Return if attempting to search by files
        if prop == SearchProp::Files {
            return Err(raur::Error::Aur(String::from("Cannot search by files.")))
        }

        // Set search mode
        let search_by = match prop {
            SearchProp::Name => { raur::SearchBy::Name },
            SearchProp::NameDesc => { raur::SearchBy::NameDesc },
            SearchProp::Groups => { raur::SearchBy::Groups },
            SearchProp::Deps => { raur::SearchBy::Depends },
            SearchProp::Optdeps => { raur::SearchBy::OptDepends },
            SearchProp::Provides => { raur::SearchBy::Provides },
            SearchProp::Files => unreachable!(),
        };

        // Search for AUR packages
        let search_results = future::join_all(term.split_whitespace()
            .map(|t| handle.search_by(t, search_by))
        )
        .await;

        let mut aur_names: HashSet<String> = HashSet::new();

        println!("{:?}",installed_pkg_names.len());

        for result in search_results {
            aur_names.extend(result?.iter()
                .filter(|&pkg| !installed_pkg_names.contains(&pkg.name))
                .map(|pkg| pkg.name.clone())
            );
        }

        // Get AUR package info using cache
        let mut cache = aur_cache.lock().await;

        let aur_list = handle.cache_info(&mut cache, &aur_names.iter().collect::<Vec<&String>>())
            .await?;

        Ok(aur_list)
    }

    //---------------------------------------
    // Public search in AUR function
    //---------------------------------------
    pub fn search_in_aur(&self, search_bar: SearchBar, search_term: &str, prop: SearchProp) {
        let imp = self.imp();

        // Cancel ongoing AUR search if any
        self.cancel_aur_search();

        // Clear AUR search results
        imp.aur_model.remove_all();

        AUR_PKGS.replace(vec![]);

        // Return if search term is empty
        if search_term.is_empty() {
            return
        }

        let term = search_term.to_lowercase();

        // Show search spinner
        search_bar.set_searching(true);

        // Get AUR cache (clone Arc)
        let aur_cache = Arc::clone(&imp.aur_cache);

        // Create and store search cancel token
        let cancel_token = CancellationToken::new();

        let cancel_token_clone = cancel_token.clone();

        imp.search_cancel_token.replace(Some(cancel_token));

        // Spawn tokio task to search AUR
        let (sender, receiver) = async_channel::bounded(1);

        INSTALLED_PKG_NAMES.with_borrow(|installed_pkg_names| {
            Tokio::runtime().spawn(clone!(
                #[strong] installed_pkg_names,
                async move {
                    let result = tokio::select! {
                        () = cancel_token_clone.cancelled() => { Ok(vec![]) },
                        res = Self::do_search_async(&term, prop, &installed_pkg_names, &aur_cache) => {
                            res.map(|aur_list|
                                aur_list.iter().map(|pkg| PkgData::from_aur(pkg)).collect()
                            )
                        }
                    };

                    sender.send(result)
                        .await
                        .expect("Failed to send through channel");
                }
            ));
        });

        // Attach channel receiver
        glib::spawn_future_local(clone!(
            #[weak] imp,
            async move {
                while let Ok(result) = receiver.recv().await {
                    match result {
                        // Get AUR search results
                        Ok(data_list) => {
                            if search_bar.enabled() {
                                let pkg_list: Vec<PkgObject> = data_list.into_iter()
                                    .map(|data| PkgObject::new(data, None))
                                    .collect();

                                imp.aur_model.splice(0, imp.aur_model.n_items(), &pkg_list);

                                AUR_PKGS.replace(pkg_list);
                            }

                            search_bar.set_aur_error(None);
                        },
                        Err(error) => {
                            search_bar.set_aur_error(Some(error.to_string()));
                        }
                    }

                    // Remove stored search cancel token
                    imp.search_cancel_token.replace(None);

                    // Hide search spinner
                    search_bar.set_searching(false);
                }
            }
        ));
    }

    //---------------------------------------
    // Public cancel AUR search function
    //---------------------------------------
    pub fn cancel_aur_search(&self) {
        let imp = self.imp();

        if let Some(token) = imp.search_cancel_token.take() {
            token.cancel();
        }
    }

    //---------------------------------------
    // Public view function
    //---------------------------------------
    pub fn view(&self) -> gtk::ListView {
        self.imp().view.get()
    }

    //---------------------------------------
    // Public set status function
    //---------------------------------------
    pub fn set_status(&self, status: PackageViewStatus) {
        let imp = self.imp();

        match status {
            PackageViewStatus::Normal => {
                imp.stack.set_visible_child_name("view");
            },
            PackageViewStatus::PackageLoad => {
                imp.loading_status.set_title("Loading Pacman Databases");
                imp.stack.set_visible_child_name("spinner");
            },
            PackageViewStatus::AURDownload => {
                imp.loading_status.set_title("Updating AUR Database");
                imp.stack.set_visible_child_name("spinner");
            }
        }
    }

    //---------------------------------------
    // Public package functions
    //---------------------------------------
    pub fn splice_packages(&self, pkg_slice: &[PkgObject]) {
        let imp = self.imp();

        imp.pkg_model.splice(0, imp.pkg_model.n_items(), pkg_slice);
    }

    pub fn show_updates(&self, update_map: &HashMap<String, String>) {
        self.imp().pkg_model.iter::<PkgObject>()
            .flatten()
            .filter(|pkg| update_map.contains_key(&pkg.name()))
            .for_each(|pkg|
                pkg.set_update_version(Some(update_map[&pkg.name()].clone()))
            );
    }

    //---------------------------------------
    // Public copy list function
    //---------------------------------------
    pub fn copy_list(&self) -> String {
        let body = self.imp().selection.iter::<glib::Object>()
            .flatten()
            .map(|item| {
                let pkg = item
                    .downcast::<PkgObject>()
                    .expect("Failed to downcast to 'PkgObject'");

                format!("|{name}|{version}|{repo}|{status}|{size}|{groups}|",
                    name=pkg.name(),
                    version=pkg.version(),
                    repo=pkg.repository(),
                    status=pkg.status(),
                    size=pkg.install_size_string(),
                    groups=pkg.groups()
                )
            })
            .collect::<Vec<String>>()
            .join("\n");

        format!("## Package List\n|Package Name|Version|Repository|Status|Installed Size|Groups|\n|---|---|---|---|---:|---|\n{body}")
    }
}

impl Default for PackageView {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        Self::new()
    }
}
