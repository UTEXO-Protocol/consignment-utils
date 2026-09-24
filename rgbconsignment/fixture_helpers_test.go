package rgbconsignment

import (
	"os"
	"path/filepath"
)

func readFixture(p string) ([]byte, error) {
	return os.ReadFile(filepath.FromSlash(p))
}
