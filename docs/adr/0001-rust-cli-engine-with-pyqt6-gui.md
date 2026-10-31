# Rust CLI Engine with PyQt6 GUI Frontend

Status: accepted

`downsort` splits its architecture into a zero-dependency Rust CLI engine (`downsort`) and a thin `PyQt6` desktop GUI frontend invoked via `downsort --gui`. We chose this split because the core sorting pipeline benefits from a fast, self-contained compiled binary that builds and tests offline without external crates, while the target desktop environment (KDE Plasma on Linux) already provides system `PyQt6` bindings for a native Qt6 interface without pulling heavy GUI toolkits into the Rust build graph.
