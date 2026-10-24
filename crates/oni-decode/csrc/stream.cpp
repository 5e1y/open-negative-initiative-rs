// LibRaw reading a file through a handle the caller opened: the name never meets the ANSI code page
// nor MAX_PATH, and the handle keeps the file shared, so a rename or a move is never refused.
#include "libraw/libraw.h"
#include <stdint.h>
#include <stdio.h>
#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#include <windows.h>
#else
#include <unistd.h>
#endif

namespace {

// LibRaw's own FILE* stream, the one it reads files over 250 MB with, handed an open FILE*.
class handle_stream : public LibRaw_bigfile_datastream
{
public:
    explicit handle_stream(FILE *file) : LibRaw_bigfile_datastream("")
    {
        f = file;
        _fsize = 0;
        if (seek(0, SEEK_END) == 0) _fsize = tell();
        seek(0, SEEK_SET);
    }
};

} // namespace

// The stream over `handle`, a Windows HANDLE or a POSIX descriptor, which it owns from this call
// on, closed on failure too; NULL when it cannot be read.
extern "C" void *oni_stream(intptr_t handle)
{
#ifdef _WIN32
    int fd = _open_osfhandle(handle, _O_RDONLY | _O_BINARY);
    if (fd < 0) { CloseHandle((HANDLE)handle); return NULL; }
    FILE *file = _fdopen(fd, "rb");
    if (!file) { _close(fd); return NULL; }
#else
    FILE *file = fdopen((int)handle, "rb");
    if (!file) { close((int)handle); return NULL; }
#endif
    try {
        return new handle_stream(file);
    } catch (...) {
        fclose(file);
        return NULL;
    }
}

// LibRaw opens the stream as `libraw_open_file` opens a name; the stream stays the caller's.
extern "C" int oni_stream_attach(libraw_data_t *lr, void *stream)
{
    LibRaw *raw = (LibRaw *)lr->parent_class;
    return raw->open_datastream((LibRaw_abstract_datastream *)stream);
}

// Closes the file; LibRaw must be done with it, recycled or closed.
extern "C" void oni_stream_free(void *stream)
{
    delete (LibRaw_abstract_datastream *)stream;
}
