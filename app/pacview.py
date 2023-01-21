#!/usr/bin/env python

import gi, sys, os, datetime, urllib.parse

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, Gio, GObject, GLib, Pango

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
	def package_url(self):
		return(self.url_to_link(f'https://www.archlinux.org/packages/{self.repository}/{self.architecture}/{self.name}'))

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

	@GObject.Property(type=GObject.TYPE_STRV, default=[])
	def provides_list(self):
		return(self.pkg.provides)

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

	@GObject.Property(type=str, default="")
	def install_script(self):
		return("Yes" if self.pkg.has_scriptlet else "No")

	@GObject.Property(type=GObject.TYPE_STRV, default=[])
	def files_list(self):
		return([f[0] for f in self.local_pkg.files] if self.local_pkg is not None else [])

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

			return(f'<a href="pkg://{pkg}">{pkg}</a>{ver}{desc}')

		return('   '.join([link(s) for s in sorted(pkglist)]) if pkglist != [] else "None")

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
#-- CLASS: PKGDETAILSWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/pkgdetailswindow.ui")
class PkgDetailsWindow(Adw.PreferencesWindow):
	__gtype_name__ = "PkgDetailsWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	file_count_label = Gtk.Template.Child()
	files_label = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	_pkg_object = None

	@GObject.Property(type=PkgObject, default=None)
	def pkg_object(self):
		return(self._pkg_object)

	@pkg_object.setter
	def pkg_object(self, value):
		self._pkg_object = value

		self.file_count_label.set_text(f'{len(self.pkg_object.files_list)} files in package')
		self.files_label.set_text('\n'.join(self.pkg_object.files_list))

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

#------------------------------------------------------------------------------
#-- CLASS: PKGINFOVIEW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/pkginfoview.ui")
class PkgInfoView(Gtk.Box):
	__gtype_name__ = "PkgInfoView"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	model = Gtk.Template.Child()

	prev_button = Gtk.Template.Child()
	next_button = Gtk.Template.Child()

	pkg_detailswindow = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	_pkg_list = []
	_pkg_index = -1

	@GObject.Property(type=PkgObject, default=None)
	def pkg_object(self):
		return(self._pkg_list[0] if len(self._pkg_list) > 0 else None)

	@pkg_object.setter
	def pkg_object(self, value):
		self._pkg_list = [value]
		self._pkg_index = 0

		self.display_package(value)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

	#-----------------------------------
	# Factory signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_setup_value(self, factory, item):
		label = Gtk.Label(halign=Gtk.Align.START, wrap_mode=Pango.WrapMode.WORD, wrap=True, width_chars=30, max_width_chars=30, xalign=0, use_markup=True)
		label.connect("activate-link", self.on_link_activated)
		item.set_child(label)

	@Gtk.Template.Callback()
	def on_bind_value(self, factory, item):
		item.get_child().set_label(item.get_item().prop_value)

	#-----------------------------------
	# Link signal handler
	#-----------------------------------
	def on_link_activated(self, label, url):
		parse_url = urllib.parse.urlsplit(url)

		if parse_url.scheme != "pkg": return(False)

		pkg_name = parse_url.netloc

		pkg_dict = dict([(pkg.name, pkg) for pkg in app.pkg_objects])

		new_pkg = None

		if pkg_name in pkg_dict.keys():
			new_pkg = pkg_dict[pkg_name]
		else:
			for pkg in pkg_dict.values():
				if [s for s in pkg.provides_list if pkg_name in s] != []:
					new_pkg = pkg
					break

		if new_pkg is not None:
			self._pkg_list = self._pkg_list[:self._pkg_index+1]
			self._pkg_list.append(new_pkg)

			self._pkg_index += 1

			self.display_package(new_pkg)

		return(True)

	#-----------------------------------
	# Display functions
	#-----------------------------------
	def display_package(self, pkg_object):
		self.prev_button.set_sensitive(self._pkg_index > 0)
		self.next_button.set_sensitive(self._pkg_index < len(self._pkg_list) - 1)

		self.model.remove_all()

		if pkg_object is not None:
			self.model.append(PkgProperty("Name", f'<b>{pkg_object.name}</b>'))
			self.model.append(PkgProperty("Version", pkg_object.version))
			self.model.append(PkgProperty("Description", pkg_object.description))
			self.model.append(PkgProperty("URL", pkg_object.url))
			if pkg_object.repository in app.default_db_names: self.model.append(PkgProperty("Package URL", pkg_object.package_url))
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
			self.model.append(PkgProperty("Install Script", pkg_object.install_script))

	def display_prev_package(self):
		self._pkg_index -=1

		self.display_package(self._pkg_list[self._pkg_index])

	def display_next_package(self):
		self._pkg_index +=1

		self.display_package(self._pkg_list[self._pkg_index])

	#-----------------------------------
	# Details window function
	#-----------------------------------
	def show_details(self):
		self.pkg_detailswindow.pkg_object = self._pkg_list[self._pkg_index]

		self.pkg_detailswindow.show()

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
	selection = Gtk.Template.Child()
	sort_model = Gtk.Template.Child()
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
	_current_repo = ""
	_current_status = PkgStatus.ALL
	_current_search = ""

	@GObject.Property(type=str, default="")
	def current_repo(self):
		return(self._current_repo)

	@current_repo.setter
	def current_repo(self, value):
		self._current_repo = value

		self.repo_filter.changed(Gtk.FilterChange.DIFFERENT)

	@GObject.Property(type=int, default=PkgStatus.ALL)
	def current_status(self):
		return(self._current_status)

	@current_status.setter
	def current_status(self, value):
		self._current_status = value

		self.status_filter.changed(Gtk.FilterChange.DIFFERENT)

	@GObject.Property(type=str, default="")
	def current_search(self):
		return(self._current_search)

	@current_search.setter
	def current_search(self, value):
		self._current_search = value

		self.search_filter.changed(Gtk.FilterChange.DIFFERENT)

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
	str_id = GObject.Property(type=str, default="")

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

	header_sidebar_btn = Gtk.Template.Child()
	header_infoview_btn = Gtk.Template.Child()
	header_search_btn = Gtk.Template.Child()

	repo_listbox = Gtk.Template.Child()
	repo_listbox_all = Gtk.Template.Child()

	status_listbox = Gtk.Template.Child()
	status_listbox_all = Gtk.Template.Child()
	status_listbox_installed = Gtk.Template.Child()

	pkg_columnview = Gtk.Template.Child()
	pkg_infoview = Gtk.Template.Child()

	count_label = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Set package details window parent
		self.pkg_infoview.pkg_detailswindow.set_transient_for(self)

		# Connect header search entry to package column view
		self.header_search_entry.set_key_capture_widget(self.pkg_columnview)

		# Bind header search button state to search entry visibility
		self.header_search_btn.bind_property(
			"active",
			self.header_stack,
			"visible-child-name",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL,
			lambda binding, value: "search" if value == True else "title",
			lambda binding, value: (value == "search")
		)

		# Bind package column view selected item to info view
		self.pkg_columnview.selection.bind_property(
			"selected-item",
			self.pkg_infoview,
			"pkg_object",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT
		)

		# Bind package column view count to status label text
		self.pkg_columnview.filter_model.bind_property(
			"n-items",
			self.count_label,
			"label",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: f'{value} matching package{"s" if value != 1 else ""}'
		)

		# Add actions
		action_list = [
			( "toggle-sidebar", None, "", "true", self.toggle_sidebar_action ),
			( "toggle-infoview", None, "", "true", self.toggle_infoview_action ),
			( "search-start", self.search_start_action ),
			( "search-by-name", None, "", "true", self.search_params_action ),
			( "search-by-desc", None, "", "false", self.search_params_action ),
			( "search-by-group", None, "", "false", self.search_params_action ),
			( "search-by-deps", None, "", "false", self.search_params_action ),
			( "search-by-optdeps", None, "", "false", self.search_params_action ),
			( "search-stop", self.search_stop_action ),
			( "view-prev-package", self.view_prev_package_action ),
			( "view-next-package", self.view_next_package_action ),
			( "refresh-dbs", self.refresh_dbs_action ),
			( "show-details-window", self.show_details_window_action ),
			( "show-about", self.show_about_action ),
			( "quit-app", self.quit_app_action )
		]

		self.add_action_entries(action_list)

		# Add keyboard shortcuts
		app.set_accels_for_action("win.toggle-sidebar", ["<ctrl>b"])
		app.set_accels_for_action("win.toggle-infoview", ["<ctrl>i"])
		app.set_accels_for_action("win.search-start", ["<ctrl>f"])
		app.set_accels_for_action("win.search-stop", ["Escape"])
		app.set_accels_for_action("win.view-prev-package", ["<alt>Left"])
		app.set_accels_for_action("win.view-next-package", ["<alt>Right"])
		app.set_accels_for_action("win.refresh-dbs", ["F5"])
		app.set_accels_for_action("win.show-about", ["F1"])
		app.set_accels_for_action("win.quit-app", ["<ctrl>q"])

		# Add rows to sidebar repository list box
		self.populate_sidebar_repos()

		# Add items to package column view
		self.pkg_columnview.model.splice(0, len(self.pkg_columnview.model), app.pkg_objects)

		# Select rows in sidebar list boxes (apply repo/status filter)
		self.repo_listbox.select_row(self.repo_listbox_all)
		self.status_listbox.select_row(self.status_listbox_installed)

		# Set initial focus on package column view
		self.set_focus(self.pkg_columnview.view)

	#-----------------------------------
	# Functions
	#-----------------------------------
	def populate_sidebar_repos(self):
		while(row := self.repo_listbox.get_row_at_index(1)):
			if row != self.repo_listbox_all: self.repo_listbox.remove(row)

		for db in app.db_names:
			self.repo_listbox.append(SidebarListBoxRow(icon_name="package-x-generic-symbolic", label_text=str.title(db), str_id=db))

	#-----------------------------------
	# Action handlers
	#-----------------------------------
	def toggle_sidebar_action(self, action, value, user_data):
		action.set_state(value)

		self.header_sidebar_btn.set_active(value)

	def toggle_infoview_action(self, action, value, user_data):
		action.set_state(value)

		self.header_infoview_btn.set_active(value)

	def search_start_action(self, action, value, user_data):
		self.header_search_entry.emit("search-started")

	def search_params_action(self, action, value, user_data):
		action.set_state(value)

		prop_name = str.replace(action.props.name, "-", "_")

		self.pkg_columnview.set_property(prop_name, value)

		self.pkg_columnview.search_filter.changed(Gtk.FilterChange.DIFFERENT)

	def search_stop_action(self, action, value, user_data):
		self.header_search_entry.emit("stop-search")

	def view_prev_package_action(self, action, value, user_data):
		self.pkg_infoview.display_prev_package()

	def view_next_package_action(self, action, value, user_data):
		self.pkg_infoview.display_next_package()

	def refresh_dbs_action(self, action, value, user_data):
		app.populate_pkg_objects()
		self.populate_sidebar_repos()

		self.pkg_columnview.model.splice(0, len(self.pkg_columnview.model), app.pkg_objects)

		self.pkg_columnview.main_filter.changed(Gtk.FilterChange.DIFFERENT)

	def show_details_window_action(self, action, value, user_data):
		self.pkg_infoview.show_details()

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
		self.pkg_columnview.current_repo = row.str_id

	@Gtk.Template.Callback()
	def on_status_selected(self, listbox, row):
		self.pkg_columnview.current_status = PkgStatus(int(row.str_id))

	@Gtk.Template.Callback()
	def on_search_started(self, entry):
		self.header_stack.set_visible_child_name("search")

		self.set_focus(self.header_search_entry)

	@Gtk.Template.Callback()
	def on_search_changed(self, entry):
		self.pkg_columnview.current_search = entry.get_text().lower()

	@Gtk.Template.Callback()
	def on_search_stopped(self, entry):
		entry.set_text("")

		self.header_stack.set_visible_child_name("title")

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
		self.default_db_names = ["core", "extra", "community", "multilib"]

		db_files = list(os.listdir(db_path)) if os.path.exists(db_path) else []
		db_names = [os.path.basename(db).split(".")[0] for db in db_files if db.endswith(".db")]

		for name in self.default_db_names:
			if name in db_names:
				self.db_names.append(name)
				db_names.remove(name)

		self.db_names.extend(sorted(db_names))

		# Get pyalpm handle
		alpm_handle = pyalpm.Handle("/", alpm_folder)

		# Clear list of PkgOBjects
		self.pkg_objects.clear()

		# Build dictionary of local packages
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
