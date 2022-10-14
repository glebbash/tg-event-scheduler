FROM gitpod/workspace-full

# install Deno
RUN curl -fsSL https://deno.land/x/install/install.sh | sh
RUN echo 'export DENO_INSTALL="/home/gitpod/.deno"' >>~/.bash_profile
RUN echo 'export PATH="$DENO_INSTALL/bin:$PATH"' >>~/.bash_profile
