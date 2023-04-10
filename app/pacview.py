#!/usr/bin/env python

import gi, sys, os, urllib.parse, subprocess, shlex, re, threading, hashlib, requests
from datetime import datetime

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, Gio, GObject, Pango, Gdk, GLib

from enum import IntFlag

import pyalpm

# Global path variable
app_dir = os.path.abspath(os.path.dirname(sys.argv[0]))

# Global gresource file
gresource = Gio.Resource.load(os.path.join(app_dir, "com.github.PacView.gresource"))
gresource._register()

#------------------------------------------------------------------------------
#-- FLAGS: PKGSTATUS
#------------------------------------------------------------------------------
class PkgStatus(IntFlag):
	ALL = 31
	INSTALLED = 15
	EXPLICIT = 1
	DEPENDENCY = 2
	OPTIONAL = 4
	ORPHAN = 8
	NONE = 16
	UPDATES = 32

#------------------------------------------------------------------------------
#-- CLASS: PKGOBJECT
#------------------------------------------------------------------------------
class PkgObject(GObject.Object):
	__gtype_name__ = "PkgObject"

	#-----------------------------------
	# Internal pyalpm package properties
	#-----------------------------------
	pkg = GObject.Property(type=GObject.TYPE_PYOBJECT, default=None, flags=GObject.ParamFlags.READWRITE|GObject.ParamFlags.PRIVATE)
	localpkg = GObject.Property(type=GObject.TYPE_PYOBJECT, default=None, flags=GObject.ParamFlags.READWRITE|GObject.ParamFlags.PRIVATE)

	#-----------------------------------
	# Read-write properties
	#-----------------------------------
	status_flags = GObject.Property(type=int, default=PkgStatus.NONE)

	version = GObject.Property(type=str, default="")
	filter_repo = GObject.Property(type=str, default="")
	display_repo = GObject.Property(type=str, default="")

	has_update = GObject.Property(type=bool, default=False)

	#-----------------------------------
	# Read-only properties
	#-----------------------------------
	@GObject.Property(type=str, default="", flags=GObject.ParamFlags.READABLE)
	def name(self):
		return(self.pkg.name)

	@GObject.Property(type=str, default="", flags=GObject.ParamFlags.READABLE)
	def description(self):
		return(self.pkg.desc or "")

	@GObject.Property(type=str, default="", flags=GObject.ParamFlags.READABLE)
	def url(self):
		return(self.pkg.url or "")

	@GObject.Property(type=str, default="", flags=GObject.ParamFlags.READABLE)
	def licenses(self):
		return(', '.join(sorted(self.pkg.licenses or [])))

	@GObject.Property(type=str, default="", flags=GObject.ParamFlags.READABLE)
	def status(self):
		if self.status_flags & PkgStatus.EXPLICIT: return("explicit")
		elif self.status_flags & PkgStatus.DEPENDENCY: return("dependency")
		elif self.status_flags & PkgStatus.OPTIONAL: return("optional")
		elif self.status_flags & PkgStatus.ORPHAN: return("orphan")
		else: return("")

	@GObject.Property(type=str, default="", flags=GObject.ParamFlags.READABLE)
	def status_icon(self):
		if self.status_flags & PkgStatus.EXPLICIT: return("pkg-explicit")
		elif self.status_flags & PkgStatus.DEPENDENCY: return("pkg-dependency")
		elif self.status_flags & PkgStatus.OPTIONAL: return("pkg-optional")
		elif self.status_flags & PkgStatus.ORPHAN: return("pkg-orphan")
		else: return("")

	@GObject.Property(type=str, default="", flags=GObject.ParamFlags.READABLE)
	def group(self):
		return(', '.join(sorted(self.pkg.groups or [])))

	@GObject.Property(type=GObject.TYPE_STRV, default=[], flags=GObject.ParamFlags.READABLE)
	def provides(self):
		return(self.pkg.provides or [])

	@GObject.Property(type=GObject.TYPE_STRV, default=[], flags=GObject.ParamFlags.READABLE)
	def depends(self):
		return(self.pkg.depends or [])

	@GObject.Property(type=GObject.TYPE_STRV, default=[], flags=GObject.ParamFlags.READABLE)
	def optdepends(self):
		return(self.pkg.optdepends or [])

	@GObject.Property(type=GObject.TYPE_STRV, default=[], flags=GObject.ParamFlags.READABLE)
	def required_by(self):
		return((self.localpkg.compute_requiredby() or []) if self.localpkg is not None else (self.pkg.compute_requiredby() or []))

	@GObject.Property(type=GObject.TYPE_STRV, default=[], flags=GObject.ParamFlags.READABLE)
	def optional_for(self):
		return((self.localpkg.compute_optionalfor() or []) if self.localpkg is not None else (self.pkg.compute_optionalfor() or []))

	@GObject.Property(type=GObject.TYPE_STRV, default=[], flags=GObject.ParamFlags.READABLE)
	def conflicts(self):
		return(self.pkg.conflicts or [])

	@GObject.Property(type=GObject.TYPE_STRV, default=[], flags=GObject.ParamFlags.READABLE)
	def replaces(self):
		return(self.pkg.replaces or [])

	@GObject.Property(type=str, default="", flags=GObject.ParamFlags.READABLE)
	def architecture(self):
		return(self.pkg.arch or "")

	@GObject.Property(type=str, default="", flags=GObject.ParamFlags.READABLE)
	def packager(self):
		return(self.pkg.packager or "")

	@GObject.Property(type=str, default="", flags=GObject.ParamFlags.READABLE)
	def build_date_long(self):
		return(self.date_to_str_long(self.pkg.builddate))

	@GObject.Property(type=int, default=0, flags=GObject.ParamFlags.READABLE)
	def install_date_raw(self):
		return(self.localpkg.installdate if self.localpkg is not None else self.pkg.installdate)

	@GObject.Property(type=str, default="", flags=GObject.ParamFlags.READABLE)
	def install_date_short(self):
		return(self.date_to_str_short(self.install_date_raw))

	@GObject.Property(type=str, default="", flags=GObject.ParamFlags.READABLE)
	def install_date_long(self):
		return(self.date_to_str_long(self.install_date_raw))

	@GObject.Property(type=str, default="", flags=GObject.ParamFlags.READABLE)
	def download_size(self):
		return(self.size_to_str(self.pkg.size) if self.pkg.size != 0 else "")

	@GObject.Property(type=int, default=0, flags=GObject.ParamFlags.READABLE)
	def install_size_raw(self):
		return(self.pkg.isize)

	@GObject.Property(type=str, default="", flags=GObject.ParamFlags.READABLE)
	def install_size(self):
		return(self.size_to_str(self.pkg.isize))

	@GObject.Property(type=bool, default=False, flags=GObject.ParamFlags.READABLE)
	def install_script(self):
		return(self.pkg.has_scriptlet)

	@GObject.Property(type=GObject.TYPE_STRV, default=[], flags=GObject.ParamFlags.READABLE)
	def files(self):
		return([f[0] for f in (self.localpkg.files or [])] if self.localpkg is not None else [])

	@GObject.Property(type=GObject.TYPE_STRV, default=[], flags=GObject.ParamFlags.READABLE)
	def backup(self):
		return((self.localpkg.backup or []) if self.localpkg is not None else [])

	@GObject.Property(type=str, default="", flags=GObject.ParamFlags.READABLE)
	def sha256sum(self):
		return(self.pkg.sha256sum or "")

	@GObject.Property(type=str, default="", flags=GObject.ParamFlags.READABLE)
	def md5sum(self):
		return(self.pkg.md5sum or "")

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

	#-----------------------------------
	# Helper functions
	#-----------------------------------
	@staticmethod
	def date_to_str_short(value):
		return(datetime.fromtimestamp(value).strftime("%Y/%m/%d %H:%M") if value != 0 else "")

	@staticmethod
	def date_to_str_long(value):
		return(datetime.fromtimestamp(value).strftime("%d %B %Y %H:%M") if value != 0 else "")

	@staticmethod
	def size_to_str(value, decimals=1):
		if value == 0: return "0 B"
		
		pkg_size = value

		for unit in ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB']:
			if pkg_size < 1024.0 or unit == 'PiB':
				break
			pkg_size /= 1024.0
		
		return(f'{pkg_size:.{decimals}f} {unit}')

#------------------------------------------------------------------------------
#-- CLASS: PKGPROPERTY
#------------------------------------------------------------------------------
class PkgProperty(GObject.Object):
	__gtype_name__ = "PkgProperty"

	#-----------------------------------
	# Properties
	#-----------------------------------
	code = GObject.Property(type=str, default="")
	label = GObject.Property(type=str, default="")
	value = GObject.Property(type=str, default="")
	icon = GObject.Property(type=str, default="")

	value_binding = GObject.Property(type=GObject.Binding, default=None)
	icon_binding = GObject.Property(type=GObject.Binding, default=None)
	icon_visibile_binding = GObject.Property(type=GObject.Binding, default=None)
	link_signal_id = GObject.Property(type=int, default=0)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

#------------------------------------------------------------------------------
#-- CLASS: PKGBACKUP
#------------------------------------------------------------------------------
class PkgBackup(GObject.Object):
	__gtype_name__ = "PkgBackup"

	#-----------------------------------
	# Read/write properties
	#-----------------------------------
	filename = GObject.Property(type=str, default="")
	status_icon = GObject.Property(type=str, default="")
	status = GObject.Property(type=str, default="")

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

#------------------------------------------------------------------------------
#-- CLASS: STATSITEM
#------------------------------------------------------------------------------
class StatsItem(GObject.Object):
	__gtype_name__ = "StatsItem"

	#-----------------------------------
	# Read/write properties
	#-----------------------------------
	repository = GObject.Property(type=str, default="")
	count = GObject.Property(type=str, default="")
	size = GObject.Property(type=str, default="")

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

#------------------------------------------------------------------------------
#-- CLASS: STATSWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/statswindow.ui")
class StatsWindow(Adw.Window):
	__gtype_name__ = "StatsWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	model = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, pacman_db_names, model, *args, **kwargs):
		super().__init__(*args, **kwargs)

		def filter_installed_func(obj):
			return(obj.status_flags & PkgStatus.INSTALLED)

		def filter_db_func(obj, db):
			return(obj.filter_repo == db)

		# Initialize widgets
		total_count = 0
		total_size = 0

		installed_filter = Gtk.CustomFilter.new(filter_installed_func)
		installed_model = Gtk.FilterListModel.new(model, installed_filter)

		db_filter = Gtk.CustomFilter()
		obj_model = Gtk.FilterListModel.new(installed_model, db_filter)

		for db in pacman_db_names:
			db_filter.set_filter_func(filter_db_func, db)

			count = obj_model.get_n_items()
			total_count += count
		
			size = sum([obj.install_size_raw for obj in obj_model])
			total_size += size

			self.model.append(StatsItem(
				repository=db.title(),
				count=count,
				size=PkgObject.size_to_str(size, 2)
			))

		self.model.append(StatsItem(
			repository="<b>Total</b>",
			count=f'<b>{total_count}</b>',
			size=f'<b>{PkgObject.size_to_str(total_size, 2)}</b>'
		))

	#-----------------------------------
	# Key press signal handler
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_key_pressed(self, keyval, keycode, user_data, state):
		if keycode == Gdk.KEY_Escape and state == 0: self.close()

#------------------------------------------------------------------------------
#-- CLASS: STACKTOGGLEBUTTON
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/stacktogglebutton.ui")
class StackToggleButton(Gtk.ToggleButton):
	__gtype_name__ = "StackToggleButton"

	#-----------------------------------
	# Properties
	#-----------------------------------
	icon = GObject.Property(type=str, default="")
	text = GObject.Property(type=str, default="")
	orientation = GObject.Property(type=Gtk.Orientation, default=Gtk.Orientation.HORIZONTAL)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

#------------------------------------------------------------------------------
#-- CLASS: PKGDETAILSWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/pkgdetailswindow.ui")
class PkgDetailsWindow(Adw.ApplicationWindow):
	__gtype_name__ = "PkgDetailsWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	pkg_label = Gtk.Template.Child()

	content_stack = Gtk.Template.Child()

	files_header_label = Gtk.Template.Child()
	files_search_entry = Gtk.Template.Child()
	files_open_button = Gtk.Template.Child()
	files_copy_button = Gtk.Template.Child()
	files_view = Gtk.Template.Child()
	files_selection = Gtk.Template.Child()
	files_model = Gtk.Template.Child()
	files_filter = Gtk.Template.Child()

	tree_label = Gtk.Template.Child()
	tree_depth_label = Gtk.Template.Child()
	tree_depth_scale = Gtk.Template.Child()
	tree_reverse_button = Gtk.Template.Child()
	tree_copy_button = Gtk.Template.Child()

	log_copy_button = Gtk.Template.Child()
	log_selection = Gtk.Template.Child()
	log_model = Gtk.Template.Child()

	cache_header_label = Gtk.Template.Child()
	cache_open_button = Gtk.Template.Child()
	cache_copy_button = Gtk.Template.Child()
	cache_selection = Gtk.Template.Child()
	cache_model = Gtk.Template.Child()

	backup_header_label = Gtk.Template.Child()
	backup_open_button = Gtk.Template.Child()
	backup_copy_button = Gtk.Template.Child()
	backup_selection = Gtk.Template.Child()
	backup_model = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	pkg_object = GObject.Property(type=PkgObject, default=None)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, pkg_object, monospace_font, *args, **kwargs):
		super().__init__(*args, **kwargs)

		self.pkg_object = pkg_object

		# Initialize files search entry
		self.files_search_entry.set_key_capture_widget(self.files_view)

		# Set tree label font
		if monospace_font == "":
			gsettings = Gio.Settings(schema_id="org.gnome.desktop.interface")

			monospace_font = gsettings.get_string("monospace-font-name")

		self.tree_label.set_attributes(Pango.AttrList.from_string(f'0 -1 font-desc "{monospace_font}"'))

		# Bind file selection to file header text
		self.files_selection.bind_property(
			"n-items", self.files_header_label, "label",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: f'Files ({value})'
		)

		# Bind cache selection to cache header text
		self.cache_selection.bind_property(
			"n-items", self.cache_header_label, "label",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: f'Cache ({value})'
		)

		# Bind backup selection to backup header text
		self.backup_selection.bind_property(
			"n-items", self.backup_header_label, "label",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: f'Backup Files ({value})'
		)

		# Bind file selection to file open button state
		self.files_selection.bind_property(
			"n-items", self.files_open_button, "sensitive",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: value != 0
		)

		# Bind cache selection to cache open button state
		self.cache_selection.bind_property(
			"n-items", self.cache_open_button, "sensitive",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: value != 0
		)

		# Bind backup selection to backup open button state
		self.backup_selection.bind_property(
			"n-items", self.backup_open_button, "sensitive",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: value != 0
		)

		# Bind file selection to file copy button state
		self.files_selection.bind_property(
			"n-items", self.files_copy_button, "sensitive",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: value != 0
		)

		# Bind log selection to log copy button state
		self.log_selection.bind_property(
			"n-items", self.log_copy_button, "sensitive",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: value != 0
		)

		# Bind cache selection to cache copy button state
		self.cache_selection.bind_property(
			"n-items", self.cache_copy_button, "sensitive",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: value != 0
		)

		# Bind backup selection to backup copy button state
		self.backup_selection.bind_property(
			"n-items", self.backup_copy_button, "sensitive",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: value != 0
		)

		# Initialize widgets
		if pkg_object is not None:
			# Set package name
			self.pkg_label.set_text(f'{pkg_object.display_repo}/{pkg_object.name}')

			# Populate file list
			file_list = [f'/{f}' for f in pkg_object.files]

			self.files_model.splice(0, 0, file_list)

			# Populate dependency tree
			self.default_depth = 6

			self.populate_dep_tree(self.default_depth, False)

			# Populate log
			with open("/var/log/pacman.log", "r") as f:
				log_lines = f.readlines()

				match_expr = re.compile(f'\[(.+)T(.+)\+.+\] \[ALPM\] (installed|removed|upgraded|downgraded) {pkg_object.name} (.+)')
				sub_expr = re.compile("\[(.+)T(.+)\+.+\] (.+)\n")

				log_lines = [sub_expr.sub(r"[\1 \2]  \3", l) for l in log_lines if match_expr.match(l)]

				self.log_model.splice(0, 0, log_lines[::-1]) # Reverse list

			# Populate cache
			pkg_cache = subprocess.run(shlex.split(f'/usr/bin/paccache -vdk0 {pkg_object.name}'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

			cache_lines = [l for l in pkg_cache.stdout.decode().split('\n') if (l != "" and not l.startswith("==>") and not l.endswith(".sig"))]

			self.cache_model.splice(0, 0, cache_lines)

			# Populate backup list
			backup_list = []

			for bk in pkg_object.backup:
				filename = f'/{bk[0]}'
				src_hash = bk[1]

				md5_hash = hashlib.md5()

				try:
					with open(filename, "rb") as f:
						# Read and update hash in chunks of 4K
						for block in iter(lambda: f.read(4096), b""):
							md5_hash.update(block)
							
						text_hash = md5_hash.hexdigest()

						status_icon = "backup-unchanged" if text_hash == src_hash else "backup-changed"
						status = "unchanged" if text_hash == src_hash else "changed"
				except:
					status_icon = "backup-error"
					status = "read error"

				backup_list.append(PkgBackup(filename=filename, status_icon=status_icon, status=status))

			self.backup_model.splice(0, 0, backup_list)

	#-----------------------------------
	# Toggle button signal handler
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_stack_button_toggled(self, button):
		if button.get_active() == True:
			self.content_stack.set_visible_child_name(button.text.lower())

	#-----------------------------------
	# Files search entry signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_files_search_changed(self, entry):
		self.files_filter.set_search(entry.get_text())

	#-----------------------------------
	# Populate dependency tree function
	#-----------------------------------
	def populate_dep_tree(self, depth, reverse):
		local_flag = "" if (self.pkg_object.status_flags & PkgStatus.INSTALLED) else " -s"
		depth_flag = "" if depth == self.default_depth else f'-d {depth}'
		reverse_flag = "-r" if reverse else ""

		pkg_tree = subprocess.run(shlex.split(f'/usr/bin/pactree {local_flag} {depth_flag} {reverse_flag} {self.pkg_object.name}'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

		self.tree_label.set_label(re.sub(" provides.+", "", pkg_tree.stdout.decode()))

	#-----------------------------------
	# Dependency tree signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_tree_depth_changed(self, scale):
		if self.pkg_object is not None:
			depth = int(scale.get_value())
			
			self.populate_dep_tree(depth, self.tree_reverse_button.get_active())

			self.tree_depth_label.set_label("Default" if depth == self.default_depth else str(depth))

	@Gtk.Template.Callback()
	def on_tree_reverse_toggled(self, button):
		if self.pkg_object is not None:
			depth = int(self.tree_depth_scale.get_value())
			
			self.populate_dep_tree(depth, button.get_active())

	#-----------------------------------
	# Open file manager signal handlers
	#-----------------------------------
	def open_file_manager(self, selected_path):
		desktop = Gio.AppInfo.get_default_for_type("inode/directory", True)

		if desktop is not None:
			try:
				desktop.launch_uris_as_manager([f'file://{selected_path}'], None, GLib.SpawnFlags.DEFAULT, None, None, None, None, None)
			except:
				pass

	@Gtk.Template.Callback()
	def on_files_view_activated(self, view, pos):
		selected_item = self.files_selection.get_selected_item()

		if selected_item is not None:
			self.open_file_manager(selected_item.get_string())

	@Gtk.Template.Callback()
	def on_files_open_button_clicked(self, button):
		selected_item = self.files_selection.get_selected_item()

		if selected_item is not None:
			self.open_file_manager(selected_item.get_string())

	@Gtk.Template.Callback()
	def on_cache_view_activated(self, view, pos):
		selected_item = self.cache_selection.get_selected_item()

		if selected_item is not None:
			self.open_file_manager(f'/var/cache/pacman/pkg/{selected_item.get_string()}')

	@Gtk.Template.Callback()
	def on_cache_open_button_clicked(self, button):
		selected_item = self.cache_selection.get_selected_item()

		if selected_item is not None:
			self.open_file_manager(f'/var/cache/pacman/pkg/{selected_item.get_string()}')

	@Gtk.Template.Callback()
	def on_backup_view_activated(self, view, pos):
		selected_item = self.backup_selection.get_selected_item()

		if selected_item is not None:
			self.open_file_manager(selected_item.filename)

	@Gtk.Template.Callback()
	def on_backup_open_button_clicked(self, button):
		selected_item = self.backup_selection.get_selected_item()

		if selected_item is not None:
			self.open_file_manager(selected_item.filename)

	#-----------------------------------
	# Copy signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_files_copy_button_clicked(self, button):
		copy_text = '\n'.join([obj.get_string() for obj in self.files_selection])

		clipboard = button.get_clipboard()

		content = Gdk.ContentProvider.new_for_value(GObject.Value(str, copy_text))

		clipboard.set_content(content)

	@Gtk.Template.Callback()
	def on_tree_copy_button_clicked(self, button):
		copy_text = self.tree_label.get_label()

		clipboard = button.get_clipboard()

		content = Gdk.ContentProvider.new_for_value(GObject.Value(str, copy_text))

		clipboard.set_content(content)

	@Gtk.Template.Callback()
	def on_log_copy_button_clicked(self, button):
		copy_text = '\n'.join([obj.get_string() for obj in self.log_selection])

		clipboard = button.get_clipboard()

		content = Gdk.ContentProvider.new_for_value(GObject.Value(str, copy_text))

		clipboard.set_content(content)

	@Gtk.Template.Callback()
	def on_cache_copy_button_clicked(self, button):
		copy_text = '\n'.join([obj.get_string() for obj in self.cache_selection])

		clipboard = button.get_clipboard()

		content = Gdk.ContentProvider.new_for_value(GObject.Value(str, copy_text))

		clipboard.set_content(content)

	@Gtk.Template.Callback()
	def on_backup_copy_button_clicked(self, button):
		copy_text = '\n'.join([f'{obj.filename} ({obj.status})' for obj in self.backup_selection])

		clipboard = button.get_clipboard()

		content = Gdk.ContentProvider.new_for_value(GObject.Value(str, copy_text))

		clipboard.set_content(content)

	#-----------------------------------
	# Key press signal handler
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_key_pressed(self, keyval, keycode, user_data, state):
		if keycode == Gdk.KEY_Escape and state == 0: self.close()

#------------------------------------------------------------------------------
#-- CLASS: PREFERENCESWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/prefswindow.ui")
class PreferencesWindow(Adw.PreferencesWindow):
	__gtype_name__ = "PreferencesWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	aur_entryrow = Gtk.Template.Child()
	aur_info_image = Gtk.Template.Child()

	column_switch = Gtk.Template.Child()
	sorting_switch = Gtk.Template.Child()

	font_expander = Gtk.Template.Child()
	font_switch = Gtk.Template.Child()
	font_button = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	aur_update_command = GObject.Property(type=str, default="")

	remember_columns = GObject.Property(type=bool, default=True)
	remember_sorting = GObject.Property(type=bool, default=False)

	custom_font = GObject.Property(type=bool, default=False)
	monospace_font = GObject.Property(type=str, default="")

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Bind properties to widgets
		self.bind_property(
			"monospace_font", self.font_button, "font-desc",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL,
			lambda binding, value: Pango.FontDescription.from_string(value),
			lambda binding, value: value.to_string()
		)

		# Set aur update command info tooltip
		self.aur_info_image.set_tooltip_markup("The command should return a list of AUR updates in the format:\n\n<i>package_name  current_version</i> -> <i>new_version</i>")

		# Focus AUR entry row
		self.aur_entryrow.grab_focus_without_selecting()

	#-----------------------------------
	# Signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_reset_button_clicked(self, button):
		self.reset_dialog = Adw.MessageDialog.new(self, "Reset Preferences?", "Reset all preferences to their default values.")

		self.reset_dialog.add_response("cancel", "_Cancel")
		self.reset_dialog.add_response("reset", "_Reset")
		self.reset_dialog.set_response_appearance("reset", Adw.ResponseAppearance.DESTRUCTIVE)

		self.reset_dialog.connect("response", self.on_reset_dialog_response)

		self.reset_dialog.present()

	def on_reset_dialog_response(self, dialog, response):
		if response == "reset":
			self.load_switch.set_active(False)
			self.aur_entryrow.set_text("")
			self.column_switch.set_active(True)
			self.sorting_switch.set_active(False)
			self.font_switch.set_active(False)
			self.font_button.set_font_desc(Pango.FontDescription.from_string("Source Code Pro 11"))

		self.reset_dialog = None

#------------------------------------------------------------------------------
#-- CLASS: PKGINFOPANE
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/pkginfopane.ui")
class PkgInfoPane(Gtk.Overlay):
	__gtype_name__ = "PkgInfoPane"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	view = Gtk.Template.Child()
	model = Gtk.Template.Child()

	overlay_toolbar = Gtk.Template.Child()
	nav_button_box = Gtk.Template.Child()
	prev_button = Gtk.Template.Child()
	next_button = Gtk.Template.Child()

	empty_label = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	__obj_list = []
	__obj_index = -1

	@GObject.Property(type=PkgObject, default=None)
	def pkg_object(self):
		return(self.__obj_list[self.__obj_index] if 0 <= self.__obj_index < len(self.__obj_list) else None)

	@pkg_object.setter
	def pkg_object(self, value):
		self.__obj_list = [value]
		self.__obj_index = 0

		self.display_package(value)

		self.nav_button_box.set_visible(False)

		self.empty_label.set_visible(value is None)

	sync_db_names = GObject.Property(type=GObject.TYPE_STRV, default=[])
	pkg_model = GObject.Property(type=Gio.ListStore, default=None)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Hide column view header
		child = self.view.get_first_child()

		if child is not None and type(child).__name__ == "GtkListItemWidget":
			child.set_visible(False)

		# Bind package to overlay toolbar visibility
		self.bind_property(
			"pkg_object", self.overlay_toolbar, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: value is not None
		)

	#-----------------------------------
	# Factory signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_setup_value(self, factory, item):
		image = Gtk.Image()

		label = Gtk.Label(hexpand=True, vexpand=True, xalign=0, yalign=0, use_markup=True, can_focus=False, selectable=True, wrap=True, margin_end=32)

		box = Gtk.Box(margin_start=4, spacing=6)
		box.append(image)
		box.append(label)

		item.set_child(box)

	@Gtk.Template.Callback()
	def on_bind_value(self, factory, item):
		prop = item.get_item()
		
		box = item.get_child()
		image = box.get_first_child()
		label = box.get_last_child()

		prop.icon_visibile_binding = prop.bind_property(
			"icon", image, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: value != ""
		)

		prop.icon_binding = prop.bind_property(
			"icon", image, "icon_name",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT
		)

		prop.value_binding = prop.bind_property(
			"value", label, "label",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT
		)

		prop.link_signal_id = label.connect("activate-link", self.on_link_activated)

	@Gtk.Template.Callback()
	def on_unbind_value(self, factory, item):
		prop = item.get_item()

		box = item.get_child()
		label = box.get_last_child()

		prop.icon_visibile_binding.unbind()
		prop.icon_binding.unbind()
		prop.value_binding.unbind()

		label.disconnect(prop.link_signal_id)

		prop.icon_visibile_binding = None
		prop.icon_binding = None
		prop.value_binding = None
		prop.link_signal_id = 0

	#-----------------------------------
	# Link signal handler
	#-----------------------------------
	def on_link_activated(self, label, url):
		def filter_name_func(obj, pkg_name):
			return(obj.name == pkg_name)

		def filter_provides_func(obj, pkg_name):
			return(any(pkg_name in s for s in obj.provides))

		parse_url = urllib.parse.urlsplit(url)

		if parse_url.scheme != "pkg": return(False)

		pkg_name = parse_url.netloc

		new_obj = None

		link_filter = Gtk.CustomFilter.new(filter_name_func, pkg_name)

		link_list = Gtk.FilterListModel.new(self.pkg_model, link_filter)

		if link_list.get_n_items() > 0:
			new_obj = link_list.get_item(0)
		else:
			link_filter.set_filter_func(filter_provides_func, pkg_name)

			if link_list.get_n_items() > 0:
				new_obj = link_list.get_item(0)

		if new_obj is not None and new_obj is not self.pkg_object:
			if new_obj in self.__obj_list:
				self.__obj_index = self.__obj_list.index(new_obj)
			else:
				self.__obj_list = self.__obj_list[:self.__obj_index+1]
				self.__obj_list.append(new_obj)

				self.__obj_index += 1

			self.display_package(self.pkg_object)

			self.nav_button_box.set_visible(True)

		return(True)

	#-----------------------------------
	# Display functions
	#-----------------------------------
	def display_package(self, obj):
		self.prev_button.set_sensitive(self.__obj_index > 0)
		self.next_button.set_sensitive(self.__obj_index < len(self.__obj_list) - 1)

		self.model.remove_all()

		if obj is not None:
			self.model.append(PkgProperty(label="Name", value=f'<b>{obj.name}</b>'))
			self.model.append(PkgProperty(label="Version", value=obj.version, code="version", icon="pkg-update" if obj.has_update else ""))
			if obj.description != "": self.model.append(PkgProperty(label="Description", value=self.prop_to_string(obj.description), code="description"))
			if obj.display_repo in self.sync_db_names: self.model.append(PkgProperty(label="Package URL", value=self.prop_to_link(f'https://www.archlinux.org/packages/{obj.display_repo}/{obj.architecture}/{obj.name}')))
			elif obj.display_repo == "aur": self.model.append(PkgProperty(label="AUR URL", value=self.prop_to_link(f'https://aur.archlinux.org/packages/{obj.name}')))
			if obj.url != "": self.model.append(PkgProperty(label="URL", value=self.prop_to_link(obj.url)))
			if obj.licenses != "": self.model.append(PkgProperty(label="Licenses", value=self.prop_to_string(obj.licenses)))
			self.model.append(PkgProperty(label="Status", value=obj.status if (obj.status_flags & PkgStatus.INSTALLED) else "not installed", icon=obj.status_icon))
			self.model.append(PkgProperty(label="Repository", value=obj.display_repo, code="display_repo"))
			if obj.group != "":self.model.append(PkgProperty(label="Groups", value=obj.group))
			if obj.provides != []: self.model.append(PkgProperty(label="Provides", value=self.prop_to_wraplist(obj.provides)))
			self.model.append(PkgProperty(label="Dependencies ", value=self.prop_to_linklist(obj.depends)))
			if obj.optdepends != []: self.model.append(PkgProperty(label="Optional", value=self.prop_to_linklist(obj.optdepends)))
			self.model.append(PkgProperty(label="Required By", value=self.prop_to_linklist(obj.required_by)))
			if obj.optional_for != []: self.model.append(PkgProperty(label="Optional For", value=self.prop_to_linklist(obj.optional_for)))
			if obj.conflicts != []: self.model.append(PkgProperty(label="Conflicts With", value=self.prop_to_linklist(obj.conflicts)))
			if obj.replaces != []: self.model.append(PkgProperty(label="Replaces", value=self.prop_to_linklist(obj.replaces)))
			if obj.architecture != "": self.model.append(PkgProperty(label="Architecture", value=obj.architecture))
			if obj.packager != "": self.model.append(PkgProperty(label="Packager", value=self.prop_to_packager(obj.packager)))
			self.model.append(PkgProperty(label="Build Date", value=obj.build_date_long))
			if obj.install_date_long != "": self.model.append(PkgProperty(label="Install Date", value=obj.install_date_long))
			if obj.download_size != "": self.model.append(PkgProperty(label="Download Size", value=obj.download_size))
			self.model.append(PkgProperty(label="Installed Size", value=obj.install_size))
			self.model.append(PkgProperty(label="Install Script", value="Yes" if obj.install_script else "No"))
			if obj.sha256sum != "": self.model.append(PkgProperty(label="SHA256 Sum", value=obj.sha256sum))
			if obj.md5sum != "": self.model.append(PkgProperty(label="MD5 Sum", value=obj.md5sum))

	def display_prev_package(self):
		if self.__obj_index > 0:
			self.__obj_index -=1

			self.display_package(self.pkg_object)

	def display_next_package(self):
		if self.__obj_index < len(self.__obj_list) - 1:
			self.__obj_index +=1

			self.display_package(self.pkg_object)

	#-----------------------------------
	# Helper functions
	#-----------------------------------
	@staticmethod
	def prop_to_string(string):
		return(GLib.markup_escape_text(string))

	@staticmethod
	def prop_to_link(url):
		escaped_url = GLib.markup_escape_text(url)
		return(f'<a href="{escaped_url}">{escaped_url}</a>')

	@staticmethod
	def prop_to_packager(email):
		if re.match("([^<]+)<([^>]+)>", email):
			return(re.sub("([^<]+)<?([^>]+)?>?", r"\1&lt;<a href='mailto:\2'>\2</a>&gt;", email))
		else:
			return(email)

	@staticmethod
	def prop_to_wraplist(pkglist, wrap_width=150):
		return(GLib.markup_escape_text('   '.join(sorted(pkglist))))

	@staticmethod
	def prop_to_linklist(pkglist):
		if pkglist == []: return("None")

		match_expr = "(^|   |   \n)([a-zA-Z0-9@._+-]+)(?=&gt;|&lt;|<|>|=|:|   |\n|$)"
		join_str = PkgInfoPane.prop_to_wraplist(pkglist)

		return(re.sub(match_expr, r"\1<a href='pkg://\2'>\2</a>", join_str))

#------------------------------------------------------------------------------
#-- CLASS: PKGCOLUMNVIEW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/pkgcolumnview.ui")
class PkgColumnView(Gtk.Overlay):
	__gtype_name__ = "PkgColumnView"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	view = Gtk.Template.Child()
	selection = Gtk.Template.Child()
	filter_model = Gtk.Template.Child()
	model = Gtk.Template.Child()

	click_gesture = Gtk.Template.Child()
	popover_menu = Gtk.Template.Child()

	empty_label = Gtk.Template.Child()
	loading_box = Gtk.Template.Child()
	loading_spinner = Gtk.Template.Child()

	repo_filter = Gtk.Template.Child()
	status_filter = Gtk.Template.Child()
	search_filter = Gtk.Template.Child()

	package_column = Gtk.Template.Child()
	version_column = Gtk.Template.Child()
	repository_column = Gtk.Template.Child()
	status_column = Gtk.Template.Child()
	date_column = Gtk.Template.Child()
	size_column = Gtk.Template.Child()
	group_column = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	is_loading = GObject.Property(type=bool, default=True)

	column_ids = GObject.Property(type=GObject.TYPE_STRV, default=[])
	default_column_ids = GObject.Property(type=GObject.TYPE_STRV, default=[])
	sort_id = GObject.Property(type=str, default="")
	default_sort_id = GObject.Property(type=str, default="")
	sort_asc = GObject.Property(type=bool, default=True)

	current_status = GObject.Property(type=int, default=PkgStatus.ALL)

	current_search = GObject.Property(type=str, default="")
	search_exact = GObject.Property(type=bool, default=False)

	search_by_name = GObject.Property(type=bool, default=True)
	search_by_desc = GObject.Property(type=bool, default=False)
	search_by_group = GObject.Property(type=bool, default=False)
	search_by_deps = GObject.Property(type=bool, default=False)
	search_by_optdeps = GObject.Property(type=bool, default=False)
	search_by_provides = GObject.Property(type=bool, default=False)
	search_by_files = GObject.Property(type=bool, default=False)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Bind item count to empty label visibility
		self.selection.bind_property(
			"n-items", self.empty_label, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: self.is_loading == False and value == 0
		)

		# Bind is_loading property to loading box visibility
		self.bind_property(
			"is_loading", self.loading_box, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT
		)

		# Set filter functions
		self.status_filter.set_filter_func(self.filter_by_status)
		self.search_filter.set_filter_func(self.filter_by_search)

		# Connect property change signal handlers
		self.connect("notify::current-status", self.on_current_status_changed)

		self.connect("notify::current-search", self.on_current_search_changed)
		self.connect("notify::search-exact", self.on_current_search_changed)

		self.connect("notify::search-by-name", self.on_current_search_changed)
		self.connect("notify::search-by-desc", self.on_current_search_changed)
		self.connect("notify::search-by-group", self.on_current_search_changed)
		self.connect("notify::search-by-deps", self.on_current_search_changed)
		self.connect("notify::search-by-optdeps", self.on_current_search_changed)
		self.connect("notify::search-by-provides", self.on_current_search_changed)
		self.connect("notify::search-by-files", self.on_current_search_changed)

	#-----------------------------------
	# Filter functions
	#-----------------------------------
	def filter_by_status(self, item):
		return(item.status_flags & self.current_status)

	def filter_by_search(self, item):
		if self.current_search == "":
			return(True)
		else:
			search_term = self.current_search.lower()

			if self.search_exact == True:
				return(any((
					self.search_by_name and search_term == item.name.lower(),
					self.search_by_desc and search_term == item.description.lower(),
					self.search_by_group and search_term == item.group.lower(),
					self.search_by_deps and any(search_term == s.lower() for s in item.depends),
					self.search_by_optdeps and any(search_term == s.lower() for s in item.optdepends),
					self.search_by_provides and any(search_term == s.lower() for s in item.provides),
					self.search_by_files and any(search_term == s.lower() for s in item.files)
				)))
			else:
				return(any((
					self.search_by_name and search_term in item.name.lower(),
					self.search_by_desc and search_term in item.description.lower(),
					self.search_by_group and search_term in item.group.lower(),
					self.search_by_deps and any(search_term in s.lower() for s in item.depends),
					self.search_by_optdeps and any(search_term in s.lower() for s in item.optdepends),
					self.search_by_provides and any(search_term in s.lower() for s in item.provides),
					self.search_by_files and any(search_term in s.lower() for s in item.files)
				)))

	#-----------------------------------
	# Property change signal handlers
	#-----------------------------------
	def on_current_status_changed(self, view, prop):
		self.status_filter.changed(Gtk.FilterChange.DIFFERENT)

	def on_current_search_changed(self, view, prop):
		self.search_filter.changed(Gtk.FilterChange.DIFFERENT)

#------------------------------------------------------------------------------
#-- CLASS: SIDEBARLISTBOXROW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/sidebarlistboxrow.ui")
class SidebarListBoxRow(Gtk.ListBoxRow):
	__gtype_name__ = "SidebarListBoxRow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	stack = Gtk.Template.Child()
	image = Gtk.Template.Child()
	spinner = Gtk.Template.Child()
	count_box = Gtk.Template.Child()
	count_label = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	repo_id = GObject.Property(type=str, default="")
	status_id = GObject.Property(type=int, default=PkgStatus.NONE)

	icon = GObject.Property(type=str, default="")
	text = GObject.Property(type=str, default="")
	count = GObject.Property(type=str, default="")
	spinning = GObject.Property(type=bool, default=False)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Bind count property to count label visibility
		self.bind_property(
			"count", self.count_box, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: True if value != "" else False
		)

		# Bind spinning property to stack visible page
		self.bind_property(
			"spinning", self.stack, "visible_child_name",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: "spinner" if value == True else "icon"
		)

		# Bind spinning property to spinner state
		self.bind_property(
			"spinning", self.spinner, "spinning",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT
		)

#------------------------------------------------------------------------------
#-- CLASS: SEARCHTAG
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/searchtag.ui")
class SearchTag(Gtk.Box):
	__gtype_name__ = "SearchTag"

	#-----------------------------------
	# Properties
	#-----------------------------------
	text = GObject.Property(type=str, default="")

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

	#-----------------------------------
	# Signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_close_button_clicked(self, controller, n_press, x, y):
		self.set_visible(False)

#------------------------------------------------------------------------------
#-- CLASS: SEARCHHEADER
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/searchheader.ui")
class SearchHeader(Gtk.Stack):
	__gtype_name__ = "SearchHeader"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	search_entry = Gtk.Template.Child()

	searchtag_box = Gtk.Template.Child()

	filter_image = Gtk.Template.Child()
	filter_popover = Gtk.Template.Child()

	searchtag_exact = Gtk.Template.Child()
	separator_exact = Gtk.Template.Child()

	searchtag_name = Gtk.Template.Child()
	searchtag_desc = Gtk.Template.Child()
	searchtag_group = Gtk.Template.Child()
	searchtag_deps = Gtk.Template.Child()
	searchtag_optdeps = Gtk.Template.Child()
	searchtag_provides = Gtk.Template.Child()
	searchtag_files = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	title = GObject.Property(type=str, default="")

	@GObject.Property(type=Gtk.Widget, default=None)
	def key_capture_widget(self):
		return(self.search_entry.get_key_capture_widget())

	@key_capture_widget.setter
	def key_capture_widget(self, value):
		self.search_entry.set_key_capture_widget(value)

	search_active = GObject.Property(type=bool, default=False)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Bind property change signal handlers
		self.connect("notify::search-active", self.on_search_active_changed)

		# Position search tags
		Gtk.Widget.insert_after(self.searchtag_box, self.search_entry, self.search_entry.get_first_child())

	#-----------------------------------
	# Search signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_search_started(self, entry):
		self.search_active = True

	@Gtk.Template.Callback()
	def on_search_stopped(self, entry):
		self.search_active = False

	@Gtk.Template.Callback()
	def on_search_changed(self, entry):
		self.key_capture_widget.current_search = entry.get_text()

	#-----------------------------------
	# Filter signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_filter_image_clicked(self, controller, n_press, x, y):
		self.filter_popover.popup()

	#-----------------------------------
	# Property change signal handlers
	#-----------------------------------
	def on_search_active_changed(self, view, prop):
		if self.search_active == True:
			app.set_accels_for_action("win.search-exact", ["<ctrl>E"])

			app.set_accels_for_action("win.search-by-name", ["<ctrl>1"])
			app.set_accels_for_action("win.search-by-desc", ["<ctrl>2"])
			app.set_accels_for_action("win.search-by-group", ["<ctrl>3"])
			app.set_accels_for_action("win.search-by-deps", ["<ctrl>4"])
			app.set_accels_for_action("win.search-by-optdeps", ["<ctrl>5"])
			app.set_accels_for_action("win.search-by-provides", ["<ctrl>6"])
			app.set_accels_for_action("win.search-by-files", ["<ctrl>7"])

			app.set_accels_for_action("win.selectall-searchby-params", ["<ctrl>L"])
			app.set_accels_for_action("win.reset-searchby-params", ["<ctrl>R"])

			self.set_visible_child_name("search")

			self.search_entry.grab_focus()
		else:
			app.set_accels_for_action("win.search-exact", [])

			app.set_accels_for_action("win.search-by-name", [])
			app.set_accels_for_action("win.search-by-desc", [])
			app.set_accels_for_action("win.search-by-group", [])
			app.set_accels_for_action("win.search-by-deps", [])
			app.set_accels_for_action("win.search-by-optdeps", [])
			app.set_accels_for_action("win.search-by-provides", [])
			app.set_accels_for_action("win.search-by-files", [])

			app.set_accels_for_action("win.selectall-searchby-params", [])
			app.set_accels_for_action("win.reset-searchby-params", [])

			self.search_entry.set_text("")

			self.set_visible_child_name("title")

#------------------------------------------------------------------------------
#-- CLASS: MAINWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/mainwindow.ui")
class MainWindow(Adw.ApplicationWindow):
	__gtype_name__ = "MainWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	header_search = Gtk.Template.Child()

	header_sidebar_btn = Gtk.Template.Child()
	header_infopane_btn = Gtk.Template.Child()
	header_search_btn = Gtk.Template.Child()

	flap = Gtk.Template.Child()

	repo_listbox = Gtk.Template.Child()
	status_listbox = Gtk.Template.Child()

	pane = Gtk.Template.Child()

	column_view = Gtk.Template.Child()
	info_pane = Gtk.Template.Child()

	status_label = Gtk.Template.Child()

	prefs_window = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	sync_db_names = GObject.Property(type=GObject.TYPE_STRV, default=["core", "extra", "community", "multilib"])
	pacman_db_names = GObject.Property(type=GObject.TYPE_STRV, default=[])

	status_update_row = GObject.Property(type=SidebarListBoxRow, default=None)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		#-----------------------------
		# GSettings
		#-----------------------------
		# Bind gsettings
		self.gsettings = Gio.Settings(schema_id="com.github.PacView")

		self.gsettings.bind("window-width", self, "default-width", Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("window-height", self, "default-height", Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("window-maximized", self, "maximized", Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("show-sidebar", self.flap, "reveal_flap", Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("show-infopane", self.info_pane, "visible", Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("infopane-position", self.pane, "position", Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("view-columns", self.column_view, "column_ids", Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("sort-column", self.column_view, "sort_id", Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("sort-ascending", self.column_view, "sort_asc", Gio.SettingsBindFlags.DEFAULT)

		self.gsettings.bind("aur-update-command", self.prefs_window, "aur_update_command", Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("remember-columns", self.prefs_window, "remember_columns", Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("remember-sorting", self.prefs_window, "remember_sorting", Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("custom-font", self.prefs_window, "custom_font", Gio.SettingsBindFlags.DEFAULT)
		self.gsettings.bind("monospace-font", self.prefs_window, "monospace_font", Gio.SettingsBindFlags.DEFAULT)

		# Load default column order and sort column
		cols_variant = self.gsettings.get_default_value("view-columns")

		self.column_view.default_column_ids = cols_variant.get_strv()

		sort_variant = self.gsettings.get_default_value("sort-column")

		self.column_view.default_sort_id = sort_variant.get_string()

		#-----------------------------
		# Toolbar buttons
		#-----------------------------
		# Bind toolbar search button state to header search active state
		self.header_search_btn.bind_property(
			"active", self.header_search, "search_active",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		# Create toolbar button actions
		self.add_action(self.gsettings.create_action("show-sidebar"))
		self.add_action(self.gsettings.create_action("show-infopane"))

		app.set_accels_for_action("win.show-sidebar", ["<ctrl>b"])
		app.set_accels_for_action("win.show-infopane", ["<ctrl>i"])

		action_list = [
			( "search-start", self.start_search_action ),
			( "search-stop", self.stop_search_action )
		]

		self.add_action_entries(action_list)

		app.set_accels_for_action("win.search-start", ["<ctrl>f"])
		app.set_accels_for_action("win.search-stop", ["Escape"])

		#-----------------------------
		# Column view
		#-----------------------------
		# Bind column view count to status label text
		self.column_view.filter_model.bind_property(
			"n-items", self.status_label, "label",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: f'{value} matching package{"s" if value != 1 else ""}'
		)

		# Bind column view search by properties to search header tags visibility
		self.column_view.bind_property(
			"search_exact", self.header_search.searchtag_exact, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		self.column_view.bind_property(
			"search_exact", self.header_search.separator_exact, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		self.column_view.bind_property(
			"search_by_name", self.header_search.searchtag_name, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		self.column_view.bind_property(
			"search_by_desc", self.header_search.searchtag_desc, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		self.column_view.bind_property(
			"search_by_group", self.header_search.searchtag_group, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		self.column_view.bind_property(
			"search_by_deps", self.header_search.searchtag_deps, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		self.column_view.bind_property(
			"search_by_optdeps", self.header_search.searchtag_optdeps, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		self.column_view.bind_property(
			"search_by_provides", self.header_search.searchtag_provides, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		self.column_view.bind_property(
			"search_by_files", self.header_search.searchtag_files, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.BIDIRECTIONAL
		)

		# Create column view search filter actions
		self.add_action(Gio.PropertyAction.new("search-exact", self.column_view, "search_exact"))

		self.add_action(Gio.PropertyAction.new("search-by-name", self.column_view, "search_by_name"))
		self.add_action(Gio.PropertyAction.new("search-by-desc", self.column_view, "search_by_desc"))
		self.add_action(Gio.PropertyAction.new("search-by-group", self.column_view, "search_by_group"))
		self.add_action(Gio.PropertyAction.new("search-by-deps", self.column_view, "search_by_deps"))
		self.add_action(Gio.PropertyAction.new("search-by-optdeps", self.column_view, "search_by_optdeps"))
		self.add_action(Gio.PropertyAction.new("search-by-provides", self.column_view, "search_by_provides"))
		self.add_action(Gio.PropertyAction.new("search-by-files", self.column_view, "search_by_files"))

		action_list = [
			( "selectall-searchby-params", self.selectall_searchby_params_action ),
			( "reset-searchby-params", self.reset_searchby_params_action ),
			( "reset-view-columns", self.reset_view_columns_action )
		]

		self.add_action_entries(action_list)

		# Create column view header menu actions
		self.add_action(Gio.PropertyAction.new("show-column-version", self.column_view.version_column, "visible"))
		self.add_action(Gio.PropertyAction.new("show-column-repository", self.column_view.repository_column, "visible"))
		self.add_action(Gio.PropertyAction.new("show-column-status", self.column_view.status_column, "visible"))
		self.add_action(Gio.PropertyAction.new("show-column-date", self.column_view.date_column, "visible"))
		self.add_action(Gio.PropertyAction.new("show-column-size", self.column_view.size_column, "visible"))
		self.add_action(Gio.PropertyAction.new("show-column-group", self.column_view.group_column, "visible"))

		# Connect column view signals
		self.column_view.click_gesture.connect("released", self.on_column_view_clicked)
		self.column_view.view.connect("activate", self.on_column_view_activated)

		#-----------------------------
		# Info pane
		#-----------------------------
		# Bind sync db names to info pane
		self.bind_property(
			"sync_db_names", self.info_pane, "sync_db_names",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT
		)

		# Bind column view model to info pane
		self.column_view.filter_model.bind_property(
			"model", self.info_pane, "pkg_model",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT
		)

		# Bind column view selected item to info pane
		self.column_view.selection.bind_property(
			"selected-item", self.info_pane, "pkg_object",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT
		)

		# Add info pane actions
		action_list = [
			( "view-prev-package", self.view_prev_package_action ),
			( "view-next-package", self.view_next_package_action ),
			( "show-details-window", self.show_details_window_action )
		]

		self.add_action_entries(action_list)

		app.set_accels_for_action("win.view-prev-package", ["<alt>Left"])
		app.set_accels_for_action("win.view-next-package", ["<alt>Right"])
		app.set_accels_for_action("win.show-details-window", ["<alt>Return", "<alt>KP_Enter"])

		#-----------------------------
		# Window
		#-----------------------------
		# Add window actions
		action_list = [
			( "refresh-packages", self.refresh_packages_action ),
			( "show-stats-window", self.show_stats_window_action ),
			( "copy-package-list", self.copy_package_list_action ),

			( "show-preferences", self.show_preferences_action ),
			( "show-about", self.show_about_action ),
			( "quit-app", self.quit_app_action )
		]

		self.add_action_entries(action_list)

		app.set_accels_for_action("win.refresh-packages", ["F5"])
		app.set_accels_for_action("win.show-stats-window", ["<alt>S"])
		app.set_accels_for_action("win.copy-package-list", ["<alt>L"])
		
		app.set_accels_for_action("win.show-preferences", ["<ctrl>comma"])
		app.set_accels_for_action("win.show-about", ["F1"])
		app.set_accels_for_action("win.quit-app", ["<ctrl>q"])

		# Set initial focus on package column view
		self.set_focus(self.column_view.view)

	#-----------------------------------
	# Show window signal handler
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_show(self, window):
		# Set column view column order
		if self.prefs_window.remember_columns == True:
			for i,id in enumerate(self.column_view.column_ids):
				for col in self.column_view.view.get_columns():
					if col.get_id() == id: self.column_view.view.insert_column(i, col)

			for col in self.column_view.view.get_columns():
				if col.get_id() not in self.column_view.column_ids: col.set_visible(False)

		# Set column view sorting
		if self.prefs_window.remember_sorting == True:
			sort_id = self.column_view.sort_id
			sort_asc = Gtk.SortType.ASCENDING if self.column_view.sort_asc else Gtk.SortType.DESCENDING
		else:
			sort_id = self.column_view.default_sort_id
			sort_asc = Gtk.SortType.ASCENDING

		for col in self.column_view.view.get_columns():
			if col.get_id() == sort_id:
				self.column_view.view.sort_by_column(col, sort_asc)

		# Initialize window
		self.init_databases()

		self.populate_sidebar()

		# Load packages async
		load_thread = threading.Thread(target=self.load_packages_async, args=(self.pacman_db_names,), daemon=True)
		load_thread.start()

	#-----------------------------------
	# Close window signal handler
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_close(self, window):
		# Save column view column order
		if self.prefs_window.remember_columns == True:
			column_ids = []

			for col in self.column_view.view.get_columns():
				if col.get_visible() == True: column_ids.append(col.get_id())

			self.column_view.column_ids = column_ids
		else:
			self.column_view.column_ids = self.column_view.default_column_ids

		# Save column view sorting
		if self.prefs_window.remember_sorting == True:
			sorter = self.column_view.view.get_sorter()

			if (sort_col := sorter.get_primary_sort_column()) is not None:
				self.column_view.sort_id = sort_col.get_id()
			else:
				self.column_view.sort_id = ""

			self.column_view.sort_asc = True if sorter.get_primary_sort_order() == Gtk.SortType.ASCENDING else False
		else:
			self.column_view.sort_id = self.column_view.default_sort_id
			self.column_view.sort_asc = True

	#-----------------------------------
	# Init databases function
	#-----------------------------------
	def init_databases(self):
		# Get list of configured database names
		dbs = subprocess.run(shlex.split("/usr/bin/pacman-conf -l"), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

		self.pacman_db_names = [n for n in dbs.stdout.decode().split('\n') if n != ""]

		# Add foreign to configured database names
		self.pacman_db_names.append("foreign")

	#-----------------------------------
	# Populate sidebar function
	#-----------------------------------
	def populate_sidebar(self):
		# Remove rows from listboxes
		while(row := self.repo_listbox.get_row_at_index(0)):
			self.repo_listbox.remove(row)

		while(row := self.status_listbox.get_row_at_index(0)):
			self.status_listbox.remove(row)

		# Add rows to repository list box
		self.repo_listbox.append(all_row := SidebarListBoxRow(icon="repository-symbolic", text="All"))

		for db in self.pacman_db_names:
			self.repo_listbox.append(SidebarListBoxRow(icon="repository-symbolic", text=db.title(), repo_id=db))

		self.repo_listbox.select_row(all_row)

		# Add rows to status list box
		for s in PkgStatus:
			self.status_listbox.append(row := SidebarListBoxRow(icon=f'status-{s.name.lower()}-symbolic', text=s.name.title(), status_id=s.value))

			if s == PkgStatus.UPDATES:
				self.status_update_row = row
				self.status_update_row.spinning = True
				self.status_update_row.set_sensitive(False)

			if s == PkgStatus.INSTALLED:
				self.status_listbox.select_row(row)

	#-----------------------------------
	# Load packages async function
	#-----------------------------------
	def load_packages_async(self, pacman_db_names):
		# Get pyalpm handle
		alpm_handle = pyalpm.Handle("/", "/var/lib/pacman")

		# Package dict
		pkg_dict = {}

		# Add sync packages
		for db in pacman_db_names:
			sync_db = alpm_handle.register_syncdb(db, pyalpm.SIG_DATABASE_OPTIONAL)

			if sync_db is not None:
				pkg_dict.update({pkg.name: pkg for pkg in sync_db.pkgcache})

		# Add local packages
		local_db = alpm_handle.get_localdb()
		localpkg_dict = {pkg.name: pkg for pkg in local_db.pkgcache}

		pkg_dict.update({pkg.name: pkg for pkg in local_db.pkgcache if pkg.name not in pkg_dict.keys()})

		# Create list of package objects
		def __get_pkgobject(pkg):
			if pkg.name in localpkg_dict.keys():
				localpkg = localpkg_dict[pkg.name]

				if localpkg.reason == 0: status_flags = PkgStatus.EXPLICIT
				else:
					if localpkg.compute_requiredby() != []:
						status_flags = PkgStatus.DEPENDENCY
					else:
						status_flags = PkgStatus.OPTIONAL if localpkg.compute_optionalfor() != [] else PkgStatus.ORPHAN
			else:
				localpkg = None
				status_flags = PkgStatus.NONE

			repo = pkg.db.name if pkg.db.name != "local" else "foreign"

			return(PkgObject(
				pkg=pkg,
				localpkg=localpkg,
				status_flags=status_flags,
				version=localpkg.version if localpkg is not None else pkg.version,
				filter_repo=repo,
				display_repo=repo
			))

		pkg_objects = [__get_pkgobject(pkg) for pkg in pkg_dict.values()]

		# Populate column view
		GLib.idle_add(self.idle_populate_column_view, pkg_objects)

	#-----------------------------------
	# Populate column view function
	#-----------------------------------
	def idle_populate_column_view(self, pkg_objects):
		self.column_view.model.splice(0, len(self.column_view.model), pkg_objects)

		self.column_view.is_loading = False

		# Parse foreign packages async
		foreign_thread = threading.Thread(target=self.parse_foreign_pkgs_async, args=(pkg_objects,), daemon=True)
		foreign_thread.start()

		# Get updates async
		update_thread = threading.Thread(target=self.get_updates_async, args=(self.prefs_window.aur_update_command,), daemon=True)
		update_thread.start()

	#-----------------------------------
	# Parse foreign pkgs async function
	#-----------------------------------
	def parse_foreign_pkgs_async(self, pkg_objects):
		aur_list = []

		try:
			params = {"v": "5", "type": "info", "arg[]": [obj.name for obj in pkg_objects if obj.filter_repo == "foreign"]}

			response = requests.get("https://aur.archlinux.org/rpc/", params=params, timeout=5)

			if response.status_code == 200:
				data = response.json()

				if data.get("type") == "multiinfo" and data.get("results") is not None:
					aur_list = [r.get("Name", "") for r in data.get("results", [])]
		except:
			pass

		# Update repo for foreign packages
		if aur_list != []:
			GLib.idle_add(self.idle_update_foreign_pkgs, aur_list)

	#-----------------------------------
	# Update foreign pkgs function
	#-----------------------------------
	def idle_update_foreign_pkgs(self, aur_list):
		# Get model with AUR packages
		aur_filter = Gtk.CustomFilter.new(lambda obj: obj.name in aur_list)
		aur_model = Gtk.FilterListModel.new(self.column_view.model, aur_filter)

		# Update package display repository if in AUR
		for obj in aur_model:
			obj.display_repo = "aur"

			# Update info pane package properties
			if obj.name == self.info_pane.pkg_object.name:
				for i,prop in enumerate(self.info_pane.model):
					if prop.code == "display_repo":
						prop.value = obj.display_repo
					if prop.code == "description" and i < len(self.info_pane.model):
						self.info_pane.model.insert(i+1, PkgProperty(label="AUR URL", value=self.info_pane.prop_to_link(f'https://aur.archlinux.org/packages/{obj.name}')))

	#-----------------------------------
	# Get updates async function
	#-----------------------------------
	def get_updates_async(self, aur_update_command):
		# Get updates
		pacman_upd = subprocess.run(shlex.split("/usr/bin/checkupdates"), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

		update_list = pacman_upd.stdout.decode().split('\n')

		update_error = (pacman_upd.returncode == 1)

		if aur_update_command != "" and update_error == False:
			aur_upd = subprocess.run(shlex.split(aur_update_command), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

			update_list.extend(aur_upd.stdout.decode().split('\n'))

		# Build update dict
		expr = re.compile("(\S+)\s(\S+\s->\s\S+)")

		update_dict = {expr.sub(r"\1", u): expr.sub(r"\2", u) for u in update_list if expr.match(u)}

		# Show updates in sidebar
		GLib.idle_add(self.idle_show_updates, update_dict, update_error)

	#-----------------------------------
	# Show updates function
	#-----------------------------------
	def idle_show_updates(self, update_dict, update_error):
		if update_error == False and len(update_dict) != 0:
			# Get model with update packages
			update_filter = Gtk.CustomFilter.new(lambda obj: obj.name in update_dict.keys())
			update_model = Gtk.FilterListModel.new(self.column_view.model, update_filter)

			# Modify package object properties if update available
			for obj in update_model:
				obj.status_flags |= PkgStatus.UPDATES
				obj.version = update_dict[obj.name]
				obj.has_update = True

				# Update info pane package properties
				if obj.name == self.info_pane.pkg_object.name:
					for prop in self.info_pane.model:
						if prop.code == "version":
							prop.value = obj.version
							prop.icon = "pkg-update"

		# Update sidebar status listbox update row
		self.status_update_row.spinning = False
		self.status_update_row.icon = "status-updates-symbolic" if update_error == False else "status-updates-error-symbolic"
		self.status_update_row.count = f'{len(update_dict)}' if update_error == False and len(update_dict) != 0 else ""

		self.status_update_row.set_tooltip_text("" if update_error == False else "Update error")
		self.status_update_row.set_sensitive(not update_error)

		return(False)

	#-----------------------------------
	# Search action handlers
	#-----------------------------------
	def start_search_action(self, action, value, user_data):
		self.header_search.search_active = True

	def stop_search_action(self, action, value, user_data):
		self.header_search.search_active = False

		self.column_view.view.grab_focus()

	def selectall_searchby_params_action(self, action, value, user_data):
		for n in ["name", "desc", "group", "deps", "optdeps", "provides", "files"]:
			self.column_view.set_property(f'search_by_{n}', True)

	def reset_searchby_params_action(self, action, value, user_data):
		for n in ["name", "desc", "group", "deps", "optdeps", "provides", "files"]:
			self.column_view.set_property(f'search_by_{n}', (n == "name"))

	#-----------------------------------
	# Column view action handlers
	#-----------------------------------
	def reset_view_columns_action(self, action, value, user_data):
		self.column_view.column_ids = self.column_view.default_column_ids

		for i,id in enumerate(self.column_view.column_ids):
			for col in self.column_view.view.get_columns():
				if col.get_id() == id: self.column_view.view.insert_column(i, col)

		for col in self.column_view.view.get_columns():
			col.set_visible(True)

	#-----------------------------------
	# Info pane action handlers
	#-----------------------------------
	def view_prev_package_action(self, action, value, user_data):
		self.info_pane.display_prev_package()

	def view_next_package_action(self, action, value, user_data):
		self.info_pane.display_next_package()

	def show_details_window_action(self, action, value, user_data):
		if self.info_pane.pkg_object is not None:
			details_window = PkgDetailsWindow(self.info_pane.pkg_object, self.prefs_window.monospace_font if self.prefs_window.custom_font else "", transient_for=self)
			details_window.present()

	#-----------------------------------
	# Other action handlers
	#-----------------------------------
	def refresh_packages_action(self, action, value, user_data):
		self.header_search.search_active = False

		# Initialize window
		self.init_databases()

		self.populate_sidebar()

		# Load packages
		load_thread = threading.Thread(target=self.load_packages_async, args=(self.pacman_db_names,), daemon=True)
		load_thread.start()

	def show_stats_window_action(self, action, value, user_data):
		stats_window = StatsWindow(self.pacman_db_names, self.column_view.model, transient_for=self)
		stats_window.present()

	def copy_package_list_action(self, action, value, user_data):
		copy_text = '\n'.join([f'{obj.display_repo}\t{obj.name}\t{obj.version}' for obj in self.column_view.selection])

		clipboard = self.get_clipboard()

		content = Gdk.ContentProvider.new_for_value(GObject.Value(str, copy_text))

		clipboard.set_content(content)

	def show_preferences_action(self, action, value, user_data):
		self.prefs_window.set_transient_for(self)
		self.prefs_window.present()

	def show_about_action(self, action, value, user_data):
		about_window = Adw.AboutWindow(
			application_name="PacView",
			application_icon="software-properties",
			developer_name="draKKar1969",
			version="1.0.3",
			website="https://github.com/drakkar1969/pacview",
			developers=["draKKar1969"],
			designers=["draKKar1969"],
			license_type=Gtk.License.GPL_3_0,
			transient_for=self)

		about_window.present()

	def quit_app_action(self, action, value, user_data):
		self.close()

	#-----------------------------------
	# Sidebar signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_repo_selected(self, listbox, row):
		if row is not None:
			self.column_view.repo_filter.set_search(row.repo_id)

	@Gtk.Template.Callback()
	def on_status_selected(self, listbox, row):
		if row is not None:
			self.column_view.current_status = PkgStatus(row.status_id)

	#-----------------------------------
	# Column view signal handlers
	#-----------------------------------
	def on_column_view_clicked(self, controller, n_press, x, y):
		button = controller.get_current_button()

		if button == Gdk.BUTTON_PRIMARY:
			self.info_pane.pkg_object = self.column_view.selection.get_selected_item()
		elif button == Gdk.BUTTON_SECONDARY:
			rect = Gdk.Rectangle()
			rect.x = x
			rect.y = y

			self.column_view.popover_menu.set_pointing_to(rect)
			self.column_view.popover_menu.popup()

	def on_column_view_activated(self, view, position):
		self.activate_action("win.show-details-window")

#------------------------------------------------------------------------------
#-- CLASS: PACVIEWAPP
#------------------------------------------------------------------------------
class PacViewApp(Adw.Application):
	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, **kwargs):
		super().__init__(**kwargs)

	#-----------------------------------
	# Activate function
	#-----------------------------------
	def do_activate(self):
		active_window = self.get_active_window()

		if active_window:
			active_window.present()
		else:
			self.main_window = MainWindow(application=app)
			self.main_window.present()

#------------------------------------------------------------------------------
#-- MAIN APP
#------------------------------------------------------------------------------
app = PacViewApp(application_id="com.github.PacView")
app.run(sys.argv)
