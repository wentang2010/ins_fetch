#/usr/bin/bash

# need to execute this script in bash folder
pwd

#scp -r ../assets mowgliserver:/home/app/ins_fetch
# scp -r ../abi mowgliserver:/home/app/mowgli/ins_fetch
scp -r ../src mowgliserver:/home/app/ins_fetch
scp -r ../config mowgliserver:/home/app/ins_fetch
scp -r ../log mowgliserver:/home/app/ins_fetch
# scp  ../build.rs mowgliserver:/home/app/ins_fetch
scp ../Cargo.toml mowgliserver:/home/app/ins_fetch
scp ../Cargo.lock mowgliserver:/home/app/ins_fetch
# scp ../start.sh mowgliserver:/home/app/ins_fetch


# ssh mowgliserver << EOF
# cd /home/app/ins_fetch
# cargo build --release
# exit
# EOF
# echo ins_fetch compile done!

ssh mowgliserver << EOF
cd /home/app/ins_fetch
cargo build --release
./sync.sh
echo "sync finished"
cd simulate
nohup ./ins-fetch >nohup.out 2>&1 &
echo "new pid"
ps aux | grep ins-fetch | grep -v grep | awk 'system("echo "$2)'
exit
EOF
echo ins_fetch compile done!

