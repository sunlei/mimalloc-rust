/* Takes static TLS when loaded, like any library that uses the initial-exec model. */
__attribute__((tls_model("initial-exec"))) __thread char hog_tls[16];

char* hog(void) { return hog_tls; }
