
# Enet requires some win headers, however they're all lowercase in mingw so we redirect it using another header file
# echo "#include <ws2tcpip.h>" >> /usr/x86_64-w64-mingw32/include/Ws2tcpip.h
# echo "#include <mswsock.h>" >> /usr/x86_64-w64-mingw32/include/Mswsock.h

# cd /usr/x86_64-w64-mingw32/include
ln -s /usr/x86_64-w64-mingw32/include/ws2tcpip.h /usr/x86_64-w64-mingw32/include/Ws2tcpip.h
ln -s /usr/x86_64-w64-mingw32/include/mswsock.h /usr/x86_64-w64-mingw32/include/Mswsock.h