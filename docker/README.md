
# Docker Image for Moonlight-Web

Download [this folder](https://download-directory.github.io/?url=https%3A%2F%2Fgithub.com%2FMrCreativ3001%2Fmoonlight-web-stream%2Ftree%2Fmaster%2Fdocker) and then build image with:
```bash
docker build -t moonlight-web:v1.5 .
```

Run with
```bash
docker run -d -p 8080:8080 -p 40000-40100:40000-40100/udp --name moonlight-web moonlight-web:v1.5
```
or
```bash
docker run -d --net=host --name moonlight-web moonlight-web:v1.5
```