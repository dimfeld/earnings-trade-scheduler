#!/bin/bash
cat $1 | grep ^$2 |  egrep -o '(\d{4}-\d{2}-\d{2} : (.*?\[.*?\]))'
