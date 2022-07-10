#!/bin/bash

http-server -p $PORT &
yozuk-discord &
wait -n
exit $?