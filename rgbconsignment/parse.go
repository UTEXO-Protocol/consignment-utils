// Package rgbconsignment is a Go binding for the rgb-consignment Rust library.
//
// Build the native archive first (`make rust` or `go generate ./rgbconsignment`),
// then call [Parse] or [ParseFile].
package rgbconsignment

/*
#cgo CFLAGS: -I${SRCDIR}/../include
#cgo darwin,arm64 LDFLAGS: ${SRCDIR}/lib/darwin_arm64/librgb_consignment.a
#cgo darwin,amd64 LDFLAGS: ${SRCDIR}/lib/darwin_amd64/librgb_consignment.a
#cgo linux,arm64  LDFLAGS: ${SRCDIR}/lib/linux_arm64/librgb_consignment.a
#cgo linux,amd64  LDFLAGS: ${SRCDIR}/lib/linux_amd64/librgb_consignment.a
#cgo darwin LDFLAGS: -framework CoreFoundation -framework Security -liconv
#cgo linux LDFLAGS: -ldl -lm -lpthread
#include <string.h>
#include "rgb_consignment.h"
*/
import "C"

import (
	"encoding/json"
	"fmt"
	"os"
	"runtime"
	"unsafe"
)

//go:generate ../scripts/build-lib.sh

// Parse decodes a binary RGB consignment (Transfer, Contract, or Kit).
func Parse(data []byte) (*Info, error) {
	var errOut *C.char
	var ptr *C.uint8_t
	n := C.size_t(len(data))
	if n > 0 {
		ptr = (*C.uint8_t)(unsafe.Pointer(&data[0]))
	}

	raw := C.rgb_consignment_parse(ptr, n, &errOut)
	runtime.KeepAlive(data)

	if errOut != nil {
		defer C.rgb_consignment_string_free(errOut)
		return nil, fmt.Errorf("rgb consignment: %s", C.GoString(errOut))
	}
	if raw == nil {
		return nil, fmt.Errorf("rgb consignment: empty result")
	}
	defer C.rgb_consignment_string_free(raw)

	// One strlen and one copy: GoBytes lands directly in the []byte Unmarshal takes.
	buf := C.GoBytes(unsafe.Pointer(raw), C.int(C.strlen(raw)))

	var info Info
	if err := json.Unmarshal(buf, &info); err != nil {
		return nil, fmt.Errorf("rgb consignment: decode json: %w", err)
	}
	return &info, nil
}

// ParseFile reads path and parses it as a consignment.
func ParseFile(path string) (*Info, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	return Parse(data)
}
