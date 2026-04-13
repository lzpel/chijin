#include <stdio.h>
#include <stdlib.h>

// Minimal ISO-10303-21 STEP text representing a cube by its 8 corner
// CARTESIAN_POINTs.  Not a full B-Rep — just enough so a STEP reader
// can open the file and identify 8 points.  The goal of dummy_cadrum
// is to exercise the FFI + build.rs path, not STEP correctness.
char* dummy_cube_to_step(double sx, double sy, double sz) {
    const size_t cap = 4096;
    char* buf = (char*)malloc(cap);
    if (!buf) return NULL;

    int n = 0;
    n += snprintf(buf + n, cap - n,
        "ISO-10303-21;\n"
        "HEADER;\n"
        "FILE_DESCRIPTION(('dummy_cadrum cube'),'2;1');\n"
        "FILE_NAME('cube.step','',(''),(''),'dummy_cadrum','',''); \n"
        "FILE_SCHEMA(('CONFIG_CONTROL_DESIGN'));\n"
        "ENDSEC;\n"
        "DATA;\n");

    int id = 1;
    for (int i = 0; i < 2; ++i)
    for (int j = 0; j < 2; ++j)
    for (int k = 0; k < 2; ++k) {
        double x = i ? sx : 0.0;
        double y = j ? sy : 0.0;
        double z = k ? sz : 0.0;
        n += snprintf(buf + n, cap - n,
            "#%d=CARTESIAN_POINT('',(%.6f,%.6f,%.6f));\n",
            id++, x, y, z);
    }

    n += snprintf(buf + n, cap - n,
        "ENDSEC;\n"
        "END-ISO-10303-21;\n");

    return buf;
}

void dummy_free_cstring(char* s) {
    free(s);
}
