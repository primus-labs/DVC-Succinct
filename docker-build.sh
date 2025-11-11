#!/bin/bash
if [ -z "$1" ]; then
  DEFAULT_VALUE="latest" # set default
else
  DEFAULT_VALUE="$1" # use input
fi
bash bash_build.sh
docker build -t harbor.primuslabs.xyz:8081/pado/events-phala-zkvm-server:$DEFAULT_VALUE .
docker push harbor.primuslabs.xyz:8081/pado/events-phala-zkvm-server:$DEFAULT_VALUE