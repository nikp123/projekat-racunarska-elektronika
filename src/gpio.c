#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include <gpiod.h>

#ifndef	CONSUMER
#define	CONSUMER	"Rotating Table"
#endif

struct gpiod_line_request* setup_gpio(void) {
	// Statics
	const char *chipname = "/dev/gpiochip0";
	const unsigned int line_numbers[] = { 27, 22, 23, 24 };

	// Temporary variables
	struct gpiod_chip *chip;
	struct gpiod_line_settings  *line_set;
	struct gpiod_line_config    *line_cfg;
	struct gpiod_request_config *req_cfg;
	int ret;

	// Thing we actually return
	struct gpiod_line_request   *line_req = NULL;

	chip = gpiod_chip_open(chipname);
	if (!chip) {
		perror("Open chip failed\n");
		goto complete_failure;
	}

	line_set = gpiod_line_settings_new();
	if(!line_set) {
		perror("Unable to create line settings");
		goto close_chip;
	}
	gpiod_line_settings_set_direction(line_set, GPIOD_LINE_DIRECTION_OUTPUT);

	line_cfg = gpiod_line_config_new();
	if (!line_cfg) {
		perror("Failed to allocate line config\n");
		goto close_line_set;
	}

	for(size_t i = 0; i < 4; i++) {
		ret = gpiod_line_config_add_line_settings(line_cfg,
				&line_numbers[i],
				1,
				line_set);
		if(ret) {
			perror("Unable to set line");
			goto close_line_cfg;
		}
	}

	req_cfg = gpiod_request_config_new();
	if(!req_cfg) {
		goto close_line_cfg;
	}
	gpiod_request_config_set_consumer(req_cfg, CONSUMER);

	line_req = gpiod_chip_request_lines(chip, req_cfg, line_cfg);

	gpiod_request_config_free(req_cfg);
close_line_cfg:
	gpiod_line_config_free(line_cfg);
close_line_set:
	gpiod_line_settings_free(line_set);
close_chip:
	gpiod_chip_close(chip);
complete_failure:
	return line_req;
}

void set_position_gpio(struct gpiod_line_request *gpio, int position) {
	const enum gpiod_line_value values[8][4] = {
		{ GPIOD_LINE_VALUE_INACTIVE, GPIOD_LINE_VALUE_INACTIVE, GPIOD_LINE_VALUE_INACTIVE, GPIOD_LINE_VALUE_ACTIVE },
		{ GPIOD_LINE_VALUE_INACTIVE, GPIOD_LINE_VALUE_INACTIVE, GPIOD_LINE_VALUE_ACTIVE, GPIOD_LINE_VALUE_ACTIVE },
		{ GPIOD_LINE_VALUE_INACTIVE, GPIOD_LINE_VALUE_INACTIVE, GPIOD_LINE_VALUE_ACTIVE, GPIOD_LINE_VALUE_INACTIVE },
		{ GPIOD_LINE_VALUE_INACTIVE, GPIOD_LINE_VALUE_ACTIVE, GPIOD_LINE_VALUE_ACTIVE, GPIOD_LINE_VALUE_INACTIVE },
		{ GPIOD_LINE_VALUE_INACTIVE, GPIOD_LINE_VALUE_ACTIVE, GPIOD_LINE_VALUE_INACTIVE, GPIOD_LINE_VALUE_INACTIVE },
		{ GPIOD_LINE_VALUE_ACTIVE, GPIOD_LINE_VALUE_ACTIVE, GPIOD_LINE_VALUE_INACTIVE, GPIOD_LINE_VALUE_INACTIVE },
		{ GPIOD_LINE_VALUE_ACTIVE, GPIOD_LINE_VALUE_INACTIVE, GPIOD_LINE_VALUE_INACTIVE, GPIOD_LINE_VALUE_INACTIVE },
		{ GPIOD_LINE_VALUE_ACTIVE, GPIOD_LINE_VALUE_INACTIVE, GPIOD_LINE_VALUE_INACTIVE, GPIOD_LINE_VALUE_ACTIVE }
	}; 

	gpiod_line_request_set_values(gpio, values[position%8]);
}

#ifndef RUST
int main(int argc, char **argv)
{
	struct gpiod_line_request *gpio = setup_gpio();
	if(gpio == NULL) return -1;

	for(size_t i = 0; i < 4096; i++) {
		set_position_gpio(gpio, i);
		usleep(2000); // microseconds
	}

	gpiod_line_request_release(gpio);

	return 0;
}
#endif
