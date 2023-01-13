#!/usr/bin/env bash

glib-compile-resources --target=app/com.github.PacView.gresource com.github.PacView.gresource.xml

gresource list app/com.github.PacView.gresource
