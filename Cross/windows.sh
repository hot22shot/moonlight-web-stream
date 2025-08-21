cd /home

git clone https://github.com/moonlight-stream/moonlight-common-c.git
cd moonlight-common-c

mkdir build
cd build

export CMAKE_SYSROOT="/usr/x86_64-w64-mingw32"
export C_INCLUDE_PATH="/usr/x86_64-w64-mingw32/include"
export CPP_INCLUDE_PATH="/usr/x86_64-w64-mingw32/include"
export CMAKE_TOOLCHAIN_FILE="/opt/toolchain.cmake"
export CMAKE_CROSSCOMPILING=TRUE
export OPENSSL_ROOT_DIR="home/openssl"
export CMAKE_TRY_COMPILE_TARGET_TYPE="STATIC_LIBRARY"
# TODO: compile beforehand openssl
cmake -DCMAKE_BUILD_TYPE=Release ..

