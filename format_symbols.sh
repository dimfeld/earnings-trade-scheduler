#!/bin/bash
cat $1 | grep ^$2 | cut -c27-33 | egrep -o '^\S+'
