#!/usr/bin/env python

import gi, sys, os, datetime

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, Gio, GObject, GLib

import pyalpm

from enum import IntFlag

# Global path variable
app_dir = os.path.abspath(os.path.dirname(sys.argv[0]))

# Global gresource file
gresource = Gio.Resource.load(os.path.join(app_dir, "com.github.PacView.gresource"))
gresource._register()

#------------------------------------------------------------------------------
#-- FLAGS: PKGSTATUS
#------------------------------------------------------------------------------
class PkgStatus(IntFlag):
	EXPLICIT = 1
	DEPENDENCY = 2
	OPTIONAL = 4
	ORPHAN = 8
	INSTALLED = 15
	NONE = 16
	ALL = 31

#------------------------------------------------------------------------------
#-- CLASS: PKGOBJECT
#------------------------------------------------------------------------------
class PkgObject(GObject.Object):
	__gtype_name__ = "PkgObject"

	#-----------------------------------
	# Internal pyalpm package properties
	#-----------------------------------
	pkg = GObject.Property(type=GObject.TYPE_PYOBJECT, default=None)
	local_pkg = GObject.Property(type=GObject.TYPE_PYOBJECT, default=None)

	#-----------------------------------
	# Status enum property
	#-----------------------------------
	status_enum = GObject.Property(type=int, default=PkgStatus.NONE)

	#-----------------------------------
	# External read-only properties
	#-----------------------------------
	@GObject.Property(type=str, default="")
	def name(self):
		return(self.pkg.name)

	@GObject.Property(type=str, default="")
	def version(self):
		return(self.pkg.version)

	@GObject.Property(type=str, default="")
	def description(self):
		return(self.pkg.desc)

	@GObject.Property(type=str, default="")
	def url(self):
		return(self.url_to_link(self.pkg.url))

	@GObject.Property(type=str, default="")
	def licenses(self):
		return(', '.join(sorted(self.pkg.licenses)))

	@GObject.Property(type=str, default="")
	def status(self):
		str_dict = { PkgStatus.EXPLICIT: "explicit", PkgStatus.DEPENDENCY: "dependency", PkgStatus.OPTIONAL: "optional", PkgStatus.ORPHAN: "orphan" }

		return(str_dict.get(self.status_enum, ""))

	@GObject.Property(type=str, default="")
	def status_icon(self):
		icon_dict = { PkgStatus.EXPLICIT: "package-install", PkgStatus.DEPENDENCY: "package-installed-updated", PkgStatus.OPTIONAL: "package-installed-outdated", PkgStatus.ORPHAN: "package-purge" }

		return(icon_dict.get(self.status_enum, ""))

	@GObject.Property(type=str, default="")
	def repository(self):
		return(self.pkg.db.name)

	@GObject.Property(type=str, default="")
	def group(self):
		return(', '.join(sorted(self.pkg.groups)))

	@GObject.Property(type=str, default="")
	def provides(self):
		return(self.pkglist_to_linklist(self.pkg.provides))

	@GObject.Property(type=GObject.TYPE_STRV, default=[])
	def depends_list(self):
		return(self.pkg.depends)

	@GObject.Property(type=str, default="")
	def depends(self):
		return(self.pkglist_to_linklist(self.pkg.depends))

	@GObject.Property(type=GObject.TYPE_STRV, default=[])
	def optdepends_list(self):
		return(self.pkg.optdepends)

	@GObject.Property(type=str, default="")
	def optdepends(self):
		return(self.pkglist_to_linklist(self.pkg.optdepends))

	@GObject.Property(type=str, default="")
	def required_by(self):
		return(self.pkglist_to_linklist(self.local_pkg.compute_requiredby() if self.local_pkg is not None else self.pkg.compute_requiredby()))

	@GObject.Property(type=str, default="")
	def optional_for(self):
		return(self.pkglist_to_linklist(self.local_pkg.compute_optionalfor() if self.local_pkg is not None else self.pkg.compute_optionalfor()))

	@GObject.Property(type=str, default="")
	def conflicts(self):
		return(self.pkglist_to_linklist(self.pkg.conflicts))

	@GObject.Property(type=str, default="")
	def replaces(self):
		return(self.pkglist_to_linklist(self.pkg.replaces))

	@GObject.Property(type=str, default="")
	def architecture(self):
		return(self.pkg.arch)

	@GObject.Property(type=str, default="")
	def maintainer(self):
		return(GLib.markup_escape_text(self.pkg.packager))

	@GObject.Property(type=str, default="")
	def build_date_long(self):
		return(self.int_to_datestr_long(self.pkg.builddate))

	@GObject.Property(type=int, default=0)
	def install_date_raw(self):
		return(self.local_pkg.installdate if self.local_pkg is not None else self.pkg.installdate)

	@GObject.Property(type=str, default="")
	def install_date_short(self):
		return(self.int_to_datestr_short(self.install_date_raw))

	@GObject.Property(type=str, default="")
	def install_date_long(self):
		return(self.int_to_datestr_long(self.install_date_raw))

	@GObject.Property(type=str, default="")
	def download_size(self):
		return(self.int_to_sizestr(self.pkg.size) if self.local_pkg is None else "")

	@GObject.Property(type=int, default=0)
	def install_size_raw(self):
		return(self.pkg.isize)

	@GObject.Property(type=str, default="")
	def install_size(self):
		return(self.int_to_sizestr(self.pkg.isize))

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, pkg, *args, **kwargs):
		super().__init__(*args, **kwargs)

		self.pkg = pkg

	#-----------------------------------
	# Helper functions
	#-----------------------------------
	def int_to_datestr_short(self, value):
		return(datetime.datetime.fromtimestamp(value).strftime("%Y/%m/%d %H:%M") if value != 0 else "")

	def int_to_datestr_long(self, value):
		return(datetime.datetime.fromtimestamp(value).strftime("%a %d %b %Y %H:%M:%S") if value != 0 else "")

	def int_to_sizestr(self, value):
		pkg_size = value

		for unit in ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB']:
			if pkg_size < 1024.0 or unit == 'PiB':
				break
			pkg_size /= 1024.0
		
		return(f"{pkg_size:.1f} {unit}")

	def url_to_link(self, url):
		return(f'<a href="{url}">{url}</a>')

	def pkglist_to_linklist(self, pkglist):
		def link(string):
			pkg = string
			desc = ""
			ver = ""

			if ':' in pkg:
				pkg, desc = pkg.split(':', 1)
				desc = ':'+desc

			for delim in ['>=', '<=', '=', '>', '<']:
				if delim in pkg:
					pkg, ver = pkg.split(delim, 1)
					ver = delim+ver
					break

			pkg = GLib.markup_escape_text(pkg)
			desc = GLib.markup_escape_text(desc)
			ver = GLib.markup_escape_text(ver)

			return(f'<a href="{pkg}">{pkg}</a>{ver}{desc}')

		return('\n'.join([link(s) for s in sorted(pkglist)]) if pkglist != [] else "None")

#------------------------------------------------------------------------------
#-- CLASS: PKGPROPERTY
#------------------------------------------------------------------------------
class PkgProperty(GObject.Object):
	__gtype_name__ = "PkgProperty"

	#-----------------------------------
	# Read/write properties
	#-----------------------------------
	prop_name = GObject.Property(type=str, default="")
	prop_value = GObject.Property(type=str, default="")

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, name, value, *args, **kwargs):
		super().__init__(*args, **kwargs)

		self.prop_name = name
		self.prop_value = value

#------------------------------------------------------------------------------
#-- CLASS: PKGINFOGRID
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/pkginfogrid.ui")
class PkgInfoGrid(Gtk.ScrolledWindow):
	__gtype_name__ = "PkgInfoGrid"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	grid = Gtk.Template.Child()
	model = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

	#-----------------------------------
	# Display function
	#-----------------------------------
	def display_properties(self, pkg_object):
		self.model.remove_all()

		self.model.append(PkgProperty("Package", f'<b>{pkg_object.name}</b>'))
		self.model.append(PkgProperty("Version", pkg_object.version))
		self.model.append(PkgProperty("Description", pkg_object.description))
		self.model.append(PkgProperty("URL", pkg_object.url))
		self.model.append(PkgProperty("Licenses", pkg_object.licenses))
		self.model.append(PkgProperty("Status", pkg_object.status if (pkg_object.status_enum & PkgStatus.INSTALLED) else "not installed"))
		self.model.append(PkgProperty("Repository", pkg_object.repository))
		if pkg_object.group != "":self.model.append(PkgProperty("Groups", pkg_object.group))
		if pkg_object.provides != "None": self.model.append(PkgProperty("Provides", pkg_object.provides))
		self.model.append(PkgProperty("Dependencies", pkg_object.depends))
		if pkg_object.optdepends != "None": self.model.append(PkgProperty("Optional", pkg_object.optdepends))
		self.model.append(PkgProperty("Required By", pkg_object.required_by))
		if pkg_object.optional_for != "None": self.model.append(PkgProperty("Optional For", pkg_object.optional_for))
		if pkg_object.conflicts != "None": self.model.append(PkgProperty("Conflicts With", pkg_object.conflicts))
		if pkg_object.replaces != "None": self.model.append(PkgProperty("Replaces", pkg_object.replaces))
		self.model.append(PkgProperty("Architecture", pkg_object.architecture))
		self.model.append(PkgProperty("Maintainer", pkg_object.maintainer))
		self.model.append(PkgProperty("Build Date", pkg_object.build_date_long))
		if pkg_object.install_date_long != "": self.model.append(PkgProperty("Install Date", pkg_object.install_date_long))
		if pkg_object.download_size != "": self.model.append(PkgProperty("Download Size", pkg_object.download_size))
		self.model.append(PkgProperty("Installed Size", pkg_object.install_size))

#------------------------------------------------------------------------------
#-- CLASS: PKGCOLUMNVIEW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/pkgcolumnview.ui")
class PkgColumnView(Gtk.Box):
	__gtype_name__ = "PkgColumnView"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	view = Gtk.Template.Child()
	filter_model = Gtk.Template.Child()
	model = Gtk.Template.Child()

	main_filter = Gtk.Template.Child()
	repo_filter = Gtk.Template.Child()
	status_filter = Gtk.Template.Child()
	search_filter = Gtk.Template.Child()

	name_sorter = Gtk.Template.Child()
	version_sorter = Gtk.Template.Child()
	repository_sorter = Gtk.Template.Child()
	status_sorter = Gtk.Template.Child()
	date_sorter = Gtk.Template.Child()
	size_sorter = Gtk.Template.Child()
	group_sorter = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	property_grid = GObject.Property(type=PkgInfoGrid, default=None)
	status_bar = GObject.Property(type=Gtk.Statusbar, default=None)

	current_repo = GObject.Property(type=str, default="")
	current_status = GObject.Property(type=int, default=PkgStatus.ALL)
	current_search = GObject.Property(type=str, default="")

	search_by_name = GObject.Property(type=bool, default=True)
	search_by_desc = GObject.Property(type=bool, default=False)
	search_by_group = GObject.Property(type=bool, default=False)
	search_by_deps = GObject.Property(type=bool, default=False)
	search_by_optdeps = GObject.Property(type=bool, default=False)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Bind column sorters to sort functions
		self.name_sorter.set_sort_func(self.sort_by_str, "name")
		self.version_sorter.set_sort_func(self.sort_by_ver, "version")
		self.repository_sorter.set_sort_func(self.sort_by_str, "repository")
		self.status_sorter.set_sort_func(self.sort_by_str, "status")
		self.date_sorter.set_sort_func(self.sort_by_int, "install_date_raw")
		self.size_sorter.set_sort_func(self.sort_by_int, "install_size_raw")
		self.group_sorter.set_sort_func(self.sort_by_str, "group")

		# Bind filters to filter functions
		self.repo_filter.set_filter_func(self.filter_by_repo)
		self.status_filter.set_filter_func(self.filter_by_status)
		self.search_filter.set_filter_func(self.filter_by_search)

		# Sort view by name (first) column
		self.view.sort_by_column(self.view.get_columns()[0], Gtk.SortType.ASCENDING)

	#-----------------------------------
	# Sorter functions
	#-----------------------------------
	def sort_by_str(self, item_a, item_b, prop):
		prop_a = item_a.get_property(prop).lower()
		prop_b = item_b.get_property(prop).lower()

		if prop_a < prop_b: return(-1)
		else:
			if prop_a > prop_b: return(1)
			else: return(0)

	def sort_by_ver(self, item_a, item_b, prop):
		return(pyalpm.vercmp(item_a.get_property(prop), item_b.get_property(prop)))

	def sort_by_int(self, item_a, item_b, prop):
		return(item_a.get_property(prop) - item_b.get_property(prop))

	#-----------------------------------
	# Filter signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_main_filter_changed(self, change, user_data):
		n_items = self.filter_model.get_n_items()

		if self.status_bar is not None:
			self.status_bar.pop(0)
			self.status_bar.push(0, f'{n_items} matching package{"s" if n_items != 1 else ""}')

	#-----------------------------------
	# Filter functions
	#-----------------------------------
	def filter_by_repo(self, item):
		return(True if self.current_repo == "" else (item.repository.lower() == self.current_repo))

	def filter_by_status(self, item):
		return(item.status_enum & self.current_status)

	def filter_by_search(self, item):
		if self.current_search == "":
			return(True)
		else:
			match_name = (self.current_search in item.name) if self.search_by_name else False
			match_desc = (self.current_search in item.description.lower()) if self.search_by_desc else False
			match_group = (self.current_search in item.group.lower()) if self.search_by_group else False
			match_deps = ([s for s in item.depends_list if self.current_search in s] != []) if self.search_by_deps else False
			match_optdeps = ([s for s in item.optdepends_list if self.current_search in s] != []) if self.search_by_optdeps else False

			return(match_name or match_desc or match_group or match_deps or match_optdeps)

	#-----------------------------------
	# Selection signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_row_selected(self, selection, prop_name):
		if self.property_grid is not None and selection.get_selected_item() is not None:
			self.property_grid.display_properties(selection.get_selected_item())

#------------------------------------------------------------------------------
#-- CLASS: SIDEBARLISTBOXROW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/sidebarlistboxrow.ui")
class SidebarListBoxRow(Gtk.ListBoxRow):
	__gtype_name__ = "SidebarListBoxRow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	image = Gtk.Template.Child()
	label = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	str_selection = GObject.Property(type=str, default="")
	int_selection = GObject.Property(type=int, default=PkgStatus.ALL)

	@GObject.Property(type=str)
	def icon_name(self):
		return(self.image.get_icon_name())

	@icon_name.setter
	def icon_name(self, value):
		self.image.set_from_icon_name(value)

	@GObject.Property(type=str)
	def label_text(self):
		return(self.label.get_text())

	@label_text.setter
	def label_text(self, value):
		self.label.set_text(value)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

#------------------------------------------------------------------------------
#-- CLASS: MAINWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/mainwindow.ui")
class MainWindow(Adw.ApplicationWindow):
	__gtype_name__ = "MainWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	header_stack = Gtk.Template.Child()
	header_title = Gtk.Template.Child()
	header_search_box = Gtk.Template.Child()
	header_search_entry = Gtk.Template.Child()
	header_search_btn = Gtk.Template.Child()

	repo_listbox = Gtk.Template.Child()
	repo_listbox_all = Gtk.Template.Child()

	status_listbox = Gtk.Template.Child()
	status_listbox_installed = Gtk.Template.Child()

	pkg_columnview = Gtk.Template.Child()
	pkg_infogrid = Gtk.Template.Child()

	status_bar = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Set package column view property grid
		self.pkg_columnview.property_grid = self.pkg_infogrid

		# Set package column view status bar
		self.pkg_columnview.status_bar = self.status_bar

		# Add actions
		action_list = [
			( "search-start", self.search_start_action ),
			( "search-by-name", None, "", "true", self.search_params_action ),
			( "search-by-desc", None, "", "false", self.search_params_action ),
			( "search-by-group", None, "", "false", self.search_params_action ),
			( "search-by-deps", None, "", "false", self.search_params_action ),
			( "search-by-optdeps", None, "", "false", self.search_params_action ),
			( "search-stop", self.search_stop_action ),
			( "refresh-dbs", self.refresh_dbs_action ),
			( "show-about", self.show_about_action ),
			( "quit-app", self.quit_app_action )
		]

		self.add_action_entries(action_list)

		# Add keyboard shortcuts
		app.set_accels_for_action("win.search-start", ["<ctrl>f"])
		app.set_accels_for_action("win.search-stop", ["Escape"])
		app.set_accels_for_action("win.refresh-dbs", ["F5"])
		app.set_accels_for_action("win.quit-app", ["<ctrl>q"])

		# Add rows to sidebar repository list box
		self.populate_sidebar_repos()

		# Select rows in sidebar list boxes
		self.repo_listbox.select_row(self.repo_listbox_all)
		self.status_listbox.select_row(self.status_listbox_installed)

		# Connect header search entry to package column view
		self.header_search_entry.set_key_capture_widget(self.pkg_columnview)

		# Bind header search button state to search entry visibility
		self.header_search_btn.bind_property(
			"active",
			self.header_stack,
			"visible-child",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL,
			lambda binding, value: self.header_search_box if value == True else self.header_title,
			lambda binding, value: (value == self.header_search_box)
		)

		# Add items to package column view
		self.pkg_columnview.model.splice(0, len(self.pkg_columnview.model), app.pkg_objects)

		self.pkg_columnview.main_filter.changed(Gtk.FilterChange.DIFFERENT)

		# Set initial focus on package column view
		self.set_focus(self.pkg_columnview.view)

	#-----------------------------------
	# Functions
	#-----------------------------------
	def populate_sidebar_repos(self):
		while(row := self.repo_listbox.get_row_at_index(1)):
			if row != self.repo_listbox_all: self.repo_listbox.remove(row)

		for db in app.db_names:
			self.repo_listbox.append(SidebarListBoxRow(icon_name="package-x-generic-symbolic", label_text=str.title(db), str_selection=db))

	#-----------------------------------
	# Action handlers
	#-----------------------------------
	def search_start_action(self, action, value, user_data):
		self.header_search_entry.emit("search-started")

	def search_params_action(self, action, value, user_data):
		action.set_state(value)

		prop_name = str.replace(action.props.name, "-", "_")

		self.pkg_columnview.set_property(prop_name, value)

		self.pkg_columnview.search_filter.changed(Gtk.FilterChange.DIFFERENT)

	def search_stop_action(self, action, value, user_data):
		self.header_search_entry.emit("stop-search")

	def refresh_dbs_action(self, action, value, user_data):
		self.status_bar.pop(0)
		self.status_bar.push(0, "Refreshing package list...")
		
		app.populate_pkg_objects()
		self.populate_sidebar_repos()

		self.pkg_columnview.model.splice(0, len(self.pkg_columnview.model), app.pkg_objects)

		self.pkg_columnview.main_filter.changed(Gtk.FilterChange.DIFFERENT)

	def show_about_action(self, action, value, user_data):
		about_window = Adw.AboutWindow(
			application_name="PacView",
			application_icon="software-properties",
			developer_name="draKKar1969",
			version="0.0.5",
			website="https://github.com/drakkar1969/pacview",
			developers=["draKKar1969"],
			designers=["draKKar1969"],
			license_type=Gtk.License.GPL_3_0,
			transient_for=self)

		about_window.show()

	def quit_app_action(self, action, value, user_data):
		self.close()

	#-----------------------------------
	# Signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_repo_selected(self, listbox, row):
		self.pkg_columnview.current_repo = row.str_selection

		self.pkg_columnview.repo_filter.changed(Gtk.FilterChange.DIFFERENT)

	@Gtk.Template.Callback()
	def on_status_selected(self, listbox, row):
		self.pkg_columnview.current_status = row.int_selection

		self.pkg_columnview.status_filter.changed(Gtk.FilterChange.DIFFERENT)

	@Gtk.Template.Callback()
	def on_search_started(self, entry):
		self.header_stack.set_visible_child(self.header_search_box)

		self.set_focus(self.header_search_entry)

	@Gtk.Template.Callback()
	def on_search_changed(self, entry):
		self.pkg_columnview.current_search = entry.get_text().lower()

		self.pkg_columnview.search_filter.changed(Gtk.FilterChange.DIFFERENT)

	@Gtk.Template.Callback()
	def on_search_stopped(self, entry):
		entry.set_text("")

		self.header_stack.set_visible_child(self.header_title)

	@Gtk.Template.Callback()
	def on_search_btn_toggled(self, button):
		if button.get_active():
			self.header_search_entry.emit("search-started")
		else:
			self.header_search_entry.emit("stop-search")

#------------------------------------------------------------------------------
#-- CLASS: LAUNCHERAPP
#------------------------------------------------------------------------------
class LauncherApp(Adw.Application):
	#-----------------------------------
	# Variables
	#-----------------------------------
	db_names = []
	pkg_objects = []

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, **kwargs):
		super().__init__(**kwargs)

		# Connect signal handlers
		self.connect("activate", self.on_activate)

		self.populate_pkg_objects()

	def populate_pkg_objects(self):
		# Get path to pacman databases
		alpm_folder = "/var/lib/pacman"

		db_path = os.path.join(alpm_folder, "sync")

		# Build list of database names
		db_files = list(os.listdir(db_path)) if os.path.exists(db_path) else []
		self.db_names = [os.path.basename(db).split(".")[0] for db in db_files if db.endswith(".db")]

		# Get pyalpm handle
		alpm_handle = pyalpm.Handle("/", alpm_folder)

		self.pkg_objects.clear()

		# Build dictionary of names,install reasons of local packages
		local_db = alpm_handle.get_localdb()
		local_dict = dict([(pkg.name, pkg) for pkg in local_db.pkgcache])

		# Build list of PkgOBjects from packages in databases
		for db in self.db_names:
			sync_db = alpm_handle.register_syncdb(db, pyalpm.SIG_DATABASE_OPTIONAL)

			if sync_db is not None:
				self.pkg_objects.extend([PkgObject(pkg) for pkg in sync_db.pkgcache])

		# Set status for installed packages
		for i, obj in enumerate(self.pkg_objects):
			if obj.pkg.name in local_dict.keys():
				self.pkg_objects[i].local_pkg = local_dict[obj.pkg.name]

				reason = local_dict[obj.pkg.name].reason

				if reason == 0: self.pkg_objects[i].status_enum = PkgStatus.EXPLICIT
				else:
					if reason == 1:
						if local_dict[obj.pkg.name].compute_requiredby() != []:
							self.pkg_objects[i].status_enum = PkgStatus.DEPENDENCY
						else:
							self.pkg_objects[i].status_enum = PkgStatus.OPTIONAL if local_dict[obj.pkg.name].compute_optionalfor() != [] else PkgStatus.ORPHAN

	#-----------------------------------
	# Signal handlers
	#-----------------------------------
	def on_activate(self, app):
		self.main_window = MainWindow(application=app)
		self.main_window.present()

#------------------------------------------------------------------------------
#-- MAIN APP
#------------------------------------------------------------------------------
app = LauncherApp(application_id="com.github.PacView")
app.run(sys.argv)