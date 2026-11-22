# The Rust binary is built on the Ubuntu runners, so the runtime image needs a
# glibc base; Alpine's musl loader cannot start it.
FROM debian:bookworm-slim

WORKDIR /app

ARG VERSION
ARG COMMIT

# The Rust binary reads build metadata from the environment at start, so the
# image reports the tag and revision it was built from.
ENV YIN_PANEL_VERSION=$VERSION \
    YIN_PANEL_COMMIT=$COMMIT

# The service runs as an unprivileged account. The database and upload
# directories are created here and owned by it, so the shipped configuration
# works without a root-owned writable layer; bind mounts must be writable by
# UID 10001 when an operator overrides the paths.
RUN groupadd --gid 10001 yin \
    && useradd --uid 10001 --gid yin --no-create-home --shell /usr/sbin/nologin yin \
    && mkdir -p /app/database /app/uploads \
    && chown -R 10001:10001 /app/database /app/uploads

# The binary is built in one CI job and packaged in another, and it travels
# through a GitHub Actions artifact. Artifacts do not preserve the executable
# bit, so it arrives 0644 and the container refuses to start ("exists but it is
# not executable"). Setting the mode at COPY time restores it without a
# `RUN chmod` layer, which would duplicate the whole binary.
COPY --chmod=755 --chown=10001:10001 build-output/yin-panel /app/yin-panel
COPY --chown=10001:10001 runtime/web /app/web
COPY --chown=10001:10001 backend/resources/lang /app/lang
COPY --chown=10001:10001 docker/conf.yaml.example /app/conf.yaml

# The license and the third-party notices travel with every distributed copy.
# The MIT notice for the sun-panel-derived code is a condition of the MIT grant,
# so it has to be present in the image, not only in the repository.
COPY --chown=10001:10001 LICENSE THIRD-PARTY-NOTICES.md /app/

ENV TZ=Asia/Shanghai

LABEL org.opencontainers.image.title="Yin-Panel" \
      org.opencontainers.image.version=$VERSION \
      org.opencontainers.image.revision=$COMMIT \
      org.opencontainers.image.licenses="BUSL-1.1"

EXPOSE 3002

USER 10001:10001

# `bash` ships with the base image, so `/dev/tcp` gives a healthcheck without
# installing curl. Outbound TLS roots come from the Rust TLS stack.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD bash -c 'exec 3<>/dev/tcp/127.0.0.1/3002 && printf "GET / HTTP/1.0\r\n\r\n" >&3 && head -n 1 <&3 | grep -q "200"' || exit 1

CMD ["/app/yin-panel"]
