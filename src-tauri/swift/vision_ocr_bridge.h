#ifndef vision_ocr_bridge_h
#define vision_ocr_bridge_h

char *extract_text_from_image(const unsigned char *image_data,
                              int image_length);
void free_ocr_string(char *ptr);

#endif
