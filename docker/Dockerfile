FROM debian:bookworm-slim

ENV MOONLIGHT_WEB_PATH=/moonlight-web

# Create app directory
WORKDIR ${MOONLIGHT_WEB_PATH}

# Download the release binary
ADD https://github.com/MrCreativ3001/moonlight-web-stream/releases/download/v1.4/moonlight-web-x86_64-unknown-linux-gnu.zip /tmp/app.zip

# Install unzip for unzipping
RUN apt-get update && apt-get install -y unzip && \
    # Clean up the packages
    apt-get clean && rm -rf /var/lib/apt/lists/*

# Extract it
RUN unzip /tmp/app.zip -d ${MOONLIGHT_WEB_PATH} && rm /tmp/app.zip

# Copy the server directory which can contain a modified config.json and data.json if wanted
COPY ./server ${MOONLIGHT_WEB_PATH}/server

# Expose the port (adjust if you changed the config)
EXPOSE 8080 40000-40100/udp

# Mark the binaries as runnable
RUN chmod +x ${MOONLIGHT_WEB_PATH}/web-server && \
    chmod +x ${MOONLIGHT_WEB_PATH}/streamer

# Run the binary
CMD ["/moonlight-web/web-server"]