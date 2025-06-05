/*
 * Copyright (c) 2025 Stephano Cetola
 *
 * SPDX-License-Identifier: Apache-2.0
 */

#include <zephyr/kernel.h>
#include <zephyr/drivers/led_strip.h>
#include <zephyr/usb/usb_device.h>
#include <zephyr/device.h>
#include <zephyr/drivers/uart.h>
#include "pico/bootrom.h"
#include <zephyr/shell/shell.h>
#include <stdlib.h>

#include <zephyr/drivers/gpio.h>

void blink()
{
    const struct device *led_strip;
    struct led_rgb on = {255, 255, 255};
    struct led_rgb off = {0, 0, 0};

    struct led_rgb ons[7];
    struct led_rgb offs[7];
    for (int i = 0; i < 7; i++) {
        ons[i] = on;
        offs[i] = off;
    }
    led_strip = DEVICE_DT_GET(DT_ALIAS(led_strip));
    led_strip_update_rgb(led_strip, ons, 7);
    k_sleep(K_MSEC(500));
    led_strip_update_rgb(led_strip, offs, 7);
    k_sleep(K_MSEC(500));
    led_strip_update_rgb(led_strip, ons, 7);
    k_sleep(K_MSEC(500));
    led_strip_update_rgb(led_strip, offs, 7);
    k_sleep(K_MSEC(500));
}

const struct device *gpio_dev = DEVICE_DT_GET(DT_NODELABEL(gpio0));

static const uint8_t allowed_pins[] = {
    // UART(0, 1) I2C0(4, 5), I2C1(6, 7), SPI0(20-23), PWM(25), ADC(26-29), LEDSTRIP(16)
    2, 3, 8, 9, 10, 11, 12, 13, 14, 15, 17, 18, 19, 24
};

static bool is_allowed_pin(int pin)
{
    for (int i = 0; i < ARRAY_SIZE(allowed_pins); i++) {
        if (allowed_pins[i] == pin) {
            return true;
        }
    }
    return false;
}

static int cmd_setgpio(const struct shell *shell, size_t argc, char **argv)
{
    if (argc != 3) {
        shell_error(shell, "Usage: setgpio <pin> <high|low>");
        return -EINVAL;
    }

    int pin = atoi(argv[1]);
    if (pin < 0 || pin > 28) {
        shell_error(shell, "Pin number must be between 0 and 28");
        return -EINVAL;
    }

    if (!is_allowed_pin(pin)) {
        shell_error(shell, "Pin %d is not allowed to be controlled", pin);
        return -EPERM;
    }

    bool value;
    if (strcmp(argv[2], "high") == 0) {
        value = true;
    } else if (strcmp(argv[2], "low") == 0) {
        value = false;
    } else {
        shell_error(shell, "Value must be 'high' or 'low'");
        return -EINVAL;
    }

    int ret = gpio_pin_configure(gpio_dev, pin, GPIO_OUTPUT_ACTIVE);
    if (ret < 0 && ret != -EALREADY) {
        shell_error(shell, "Failed to configure pin %d as output: %d", pin, ret);
        return ret;
    }

    ret = gpio_pin_set(gpio_dev, pin, value);
    if (ret < 0) {
        shell_error(shell, "Failed to set pin %d: %d", pin, ret);
        return ret;
    }

    shell_print(shell, "Set pin %d to %s", pin, value ? "high" : "low");
    return 0;
}

static int cmd_getgpio(const struct shell *shell, size_t argc, char **argv)
{
    if (argc != 2) {
        shell_error(shell, "Usage: getgpio <pin>");
        return -EINVAL;
    }

    int pin = atoi(argv[1]);
    if (pin < 0 || pin > 28) {
        shell_error(shell, "Pin number must be between 0 and 28");
        return -EINVAL;
    }

    int ret = gpio_pin_configure(gpio_dev, pin, GPIO_INPUT);
    if (ret < 0 && ret != -EALREADY) {
        shell_error(shell, "Failed to configure pin %d as input: %d", pin, ret);
        return ret;
    }

    int val = gpio_pin_get(gpio_dev, pin);
    if (val < 0) {
        shell_error(shell, "Failed to read pin %d: %d", pin, val);
        return val;
    }

    shell_print(shell, "Pin %d is %s", pin, val ? "high" : "low");
    return 0;
}

void setup_gpio_shell(void)
{
    if (!device_is_ready(gpio_dev)) {
        printk("GPIO device not ready!\n");
    }
}

static int cmd_bootsel(const struct shell *shell, size_t argc, char **argv)
{
    blink();
    reset_usb_boot(0, 0);
    return 0;
}

static int cmd_blink(const struct shell *shell, size_t argc, char **argv)
{
    blink();
    return 0;
}


static int cmd_setled(const struct shell *shell, size_t argc, char **argv)
{
    const struct device *led_strip;
    struct led_rgb pixel = {0, 0, 0};

    if (argc < 2) {
        shell_print(shell, "Usage: setled <red|green|blue|off|custom> [value(s)]");
        return -EINVAL;
    }

    if (strcmp(argv[1], "red") == 0 || strcmp(argv[1], "green") == 0 || strcmp(argv[1], "blue") == 0) {
        int value = 255;
        if (argc == 3) {
            char *endptr;
            long val = strtol(argv[2], &endptr, 10);
            if (*endptr == '\0' && val >= 0 && val <= 255) {
                value = (int)val;
            } else {
                shell_error(shell, "Invalid brightness value. Use 0–255.");
                return -EINVAL;
            }
        }

        if (strcmp(argv[1], "red") == 0) {
            pixel.r = value;
        } else if (strcmp(argv[1], "green") == 0) {
            pixel.g = value;
        } else if (strcmp(argv[1], "blue") == 0) {
            pixel.b = value;
        }

    } else if (strcmp(argv[1], "off") == 0) {
        // All channels remain at 0
    } else if (strcmp(argv[1], "on") == 0) {
        if (argc != 3) {
            shell_error(shell, "Usage: setled on <0-255>");
            return -EINVAL;
        }

        char *endptr;
        long i = strtol(argv[2], &endptr, 10);
        if (*endptr != '\0' || i < 0 || i > 255) {
            shell_error(shell, "Invalid value. Use 0–255.");
            return -EINVAL;
        }
        pixel.r = (uint8_t)i;
        pixel.g = (uint8_t)i;
        pixel.b = (uint8_t)i;

    } else if (strcmp(argv[1], "custom") == 0) {
        if (argc != 5) {
            shell_error(shell, "Usage: setled custom <R> <G> <B>");
            return -EINVAL;
        }

        char *endptr;
        long r = strtol(argv[2], &endptr, 10);
        if (*endptr != '\0' || r < 0 || r > 255) {
            shell_error(shell, "Invalid red value. Use 0–255.");
            return -EINVAL;
        }

        long g = strtol(argv[3], &endptr, 10);
        if (*endptr != '\0' || g < 0 || g > 255) {
            shell_error(shell, "Invalid green value. Use 0–255.");
            return -EINVAL;
        }

        long b = strtol(argv[4], &endptr, 10);
        if (*endptr != '\0' || b < 0 || b > 255) {
            shell_error(shell, "Invalid blue value. Use 0–255.");
            return -EINVAL;
        }

        pixel.r = (uint8_t)r;
        pixel.g = (uint8_t)g;
        pixel.b = (uint8_t)b;

    } else {
        shell_error(shell, "Invalid color. Use red, green, blue, on, off, or custom.");
        return -EINVAL;
    }

    led_strip = DEVICE_DT_GET(DT_ALIAS(led_strip));
    if (!device_is_ready(led_strip)) {
        shell_error(shell, "LED strip device not ready");
        return -ENODEV;
    }

    struct led_rgb pixels[7];

    for (int i = 0; i < 7; i++) {
        pixels[i] = pixel;
    }

    int ret = led_strip_update_rgb(led_strip, pixels, 7);
    if (ret) {
        shell_error(shell, "Failed to update LED strip: %d", ret);
        return ret;
    }

    shell_print(shell, "LED set to %s", argv[1]);
    return 0;
}

SHELL_STATIC_SUBCMD_SET_CREATE(gpio_cmds,
    SHELL_CMD_ARG(set, NULL, "Set GPIO pin high or low. Usage: setgpio <pin> <high|low>", cmd_setgpio, 3, 0),
    SHELL_CMD_ARG(get, NULL, "Get GPIO pin state. Usage: getgpio <pin>", cmd_getgpio, 2, 0),
    SHELL_SUBCMD_SET_END
);

SHELL_CMD_REGISTER(setled, NULL, "Set LED color: red, green, blue, custom, or off", cmd_setled);

SHELL_CMD_REGISTER(gpio, &gpio_cmds, "GPIO control commands", NULL);

SHELL_CMD_REGISTER(bootsel, NULL, "Reboot into BOOTSEL mode", cmd_bootsel);

SHELL_CMD_REGISTER(blink, NULL, "182", cmd_blink);

int main(void)
{
    const struct device *dev;
    uint32_t dtr = 0;

    dev = DEVICE_DT_GET(DT_CHOSEN(zephyr_console));
    if (!device_is_ready(dev)) {
        return 0;
    }

    if (usb_enable(NULL)) {
        return 0;
    }

    /* Wait for DTR signal from the host */
    while (!dtr) {
        uart_line_ctrl_get(dev, UART_LINE_CTRL_DTR, &dtr);
        k_sleep(K_MSEC(100));
    }

    setup_gpio_shell();
    printk("USB shell started!\n");
    return 0;
}
