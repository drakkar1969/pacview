#!/usr/bin/env python

import gi, sys
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, Gio, GObject

#------------------------------------------------------------------------------
#-- CLASS: PKGOBJECT
#------------------------------------------------------------------------------
class PkgObject(GObject.Object):
	#-----------------------------------
	# Properties
	#-----------------------------------
	name = GObject.Property(type=str)
	ver = GObject.Property(type=str)
	repo = GObject.Property(type=str)
	status = GObject.Property(type=str)
	date = GObject.Property(type=str)
	size = GObject.Property(type=str)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, name="", version="", repository="", status="", date="", size="", *args, **kwargs):
		super().__init__(*args, **kwargs)

		self.name = name
		self.ver = version
		self.repo = repository
		self.status = status
		self.date = date
		self.size = size

#------------------------------------------------------------------------------
#-- CLASS: PKGCOLUMNVIEW
#------------------------------------------------------------------------------
@Gtk.Template(filename="/home/drakkar/Github/pkgview/pkgcolumnview.ui")
class PkgColumnView(Gtk.ColumnView):
	__gtype_name__ = "PkgColumnView"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	store = Gtk.Template.Child()
	col_name = Gtk.Template.Child()
	col_version = Gtk.Template.Child()
	col_repository = Gtk.Template.Child()
	col_status = Gtk.Template.Child()
	col_date = Gtk.Template.Child()
	col_size = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Bind column factories to signals
		for column in self.get_columns():
			column.get_factory().connect("setup", self.on_item_setup)
			column.get_factory().connect("bind", self.on_item_bind, column)

	#-----------------------------------
	# Factory signal handlers
	#-----------------------------------
	def on_item_setup(self, factory, item):
		item.set_child(Gtk.Label(halign=Gtk.Align.START))

	def on_item_bind(self, factory, item, column):
		if column.get_title() == "Package": item.get_child().set_label(item.get_item().name)
		if column.get_title() == "Version": item.get_child().set_label(item.get_item().ver)
		if column.get_title() == "Repository": item.get_child().set_label(item.get_item().repo)
		if column.get_title() == "Status": item.get_child().set_label(item.get_item().status)
		if column.get_title() == "Date": item.get_child().set_label(item.get_item().date)
		if column.get_title() == "Size": item.get_child().set_label(item.get_item().size)

#------------------------------------------------------------------------------
#-- CLASS: MAINWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(filename="/home/drakkar/Github/pkgview/mainwindow.ui")
class MainWindow(Adw.ApplicationWindow):
	__gtype_name__ = "MainWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	pkg_columnview = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

#------------------------------------------------------------------------------
#-- CLASS: LAUNCHERAPP
#------------------------------------------------------------------------------
class LauncherApp(Adw.Application):

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, **kwargs):
		super().__init__(**kwargs)

		# Connect signal handlers
		self.connect("activate", self.on_activate)

	#-----------------------------------
	# Signal handlers
	#-----------------------------------
	def on_activate(self, app):
		self.main_window = MainWindow(application=app)
		self.main_window.present()

#------------------------------------------------------------------------------
#-- MAIN APP
#------------------------------------------------------------------------------
app = LauncherApp(application_id="com.github.PkgView")
app.run(sys.argv)
