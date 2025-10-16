
# WIP: Docker Image for Moonlight-Web
WARNING: Streaming might not fully work because the WebRTC Peers cannot negotiate!

Download [this folder](https://download-directory.github.io/?url=https%3A%2F%2Fgithub.com%2FMrCreativ3001%2Fmoonlight-web-stream%2Ftree%2Fmaster%2FDocker) and then build image with:

```bash
docker build -t moonlight-web:v1.4 .
```

Enable the [host network feature](https://docs.docker.com/engine/network/drivers/host/).
This is required because without it WebRTC won't find a connection.

Run with:
```bash
docker run -d -p 8080:8080/tcp -p 40000-40100:40000-40100/udp moonlight-web:v1.4
```
or
```bash
docker run -d --network host moonlight-web:v1.4
```