#ifndef RGB_CONSIGNMENT_H
#define RGB_CONSIGNMENT_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Parse a binary RGB consignment (Transfer / Contract / Kit).
 *
 * On success returns a newly allocated JSON string and sets *err_out to NULL.
 * On failure returns NULL and *err_out is a newly allocated error message.
 *
 * Both strings must be freed with rgb_consignment_string_free.
 */
char *rgb_consignment_parse(const uint8_t *data, size_t len, char **err_out);

void rgb_consignment_string_free(char *s);

#ifdef __cplusplus
}
#endif

#endif /* RGB_CONSIGNMENT_H */
