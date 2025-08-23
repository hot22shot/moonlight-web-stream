ARG CROSS_BASE_IMAGE
FROM $CROSS_BASE_IMAGE AS base

# -- Overwrite the openssl finder so we just don't do anything (it'll just error out even though we have the lib when defined)
RUN echo "" > /usr/local/share/cmake-3.23/Modules/FindOpenSSL.cmake

# -- This will enable functionality for cross in the build rs
ENV MOONLIGHT_COMMON_CROSS=1