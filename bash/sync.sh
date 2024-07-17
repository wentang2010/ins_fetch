#/usr/bin/bash

echo "old pid"
ps aux | grep ins-fetch | grep -v grep | awk 'system("echo "$2"; kill -9 "$2)'
sleep 5
cp target/release/ins-fetch simulate
cp -r config simulate
cp -r log simulate

