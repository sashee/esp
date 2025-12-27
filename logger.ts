import fs from "node:fs/promises";
import {modifiedCrc, commands, bms} from "./utils.ts";
import process from "node:process";
import path from "node:path";
import { autoDetect } from "@serialport/bindings-cpp";
import { Buffer } from 'node:buffer';
import {DatabaseSync} from "node:sqlite";

const database = new DatabaseSync(path.join(process.env.HOME, "monitoring-data", "data.db"));

database.exec(`
  CREATE TABLE IF NOT EXISTS data(
    timestamp INTEGER PRIMARY KEY,
    value TEXT,
    system_uptime REAL,
    inverter_qpigs_grid_voltage REAL,
    inverter_qpigs_grid_frequency REAL,
    inverter_qpigs_ac_output_voltage REAL,
    inverter_qpigs_ac_output_frequency REAL,
    inverter_qpigs_ac_output_apparent_power REAL,
    inverter_qpigs_ac_output_active_power REAL,
    inverter_qpigs_output_load_percent REAL,
    inverter_qpigs_bus_voltage REAL,
    inverter_qpigs_battery_voltage REAL,
    inverter_qpigs_battery_charging_current REAL,
    inverter_qpigs_battery_capacity REAL,
    inverter_qpigs_inverter_heat_sink_temperature REAL,
    inverter_qpigs_pv_input_current1 REAL,
    inverter_qpigs_pv_input_voltage1 REAL,
    inverter_qpigs_battery_voltage_from_scc1 REAL,
    inverter_qpigs_battery_discharge_current REAL,
    inverter_qpigs_add_sbu_priority_version INTEGER,
    inverter_qpigs_configuration_status INTEGER,
    inverter_qpigs_scc_firmware_version INTEGER,
    inverter_qpigs_load_status INTEGER,
    inverter_qpigs_battery_voltage_to_steady_while_charging INTEGER,
    inverter_qpigs_charging_status INTEGER,
    inverter_qpigs_charging_status_scc_1 INTEGER,
    inverter_qpigs_charging_status_ac INTEGER,
    inverter_qpigs_battery_voltage_from_fans_on REAL,
    inverter_qpigs_eeprom_version INTEGER,
    inverter_qpigs_pv_charging_power1 REAL,
    inverter_qpigs_flag_for_charging_to_flating_mode INTEGER,
    inverter_qpigs_switch_on INTEGER,
    inverter_qpigs_device_status_2_reserved INTEGER,
    inverter_qpigs2_pv_input_current2 REAL,
    inverter_qpigs2_pv_input_voltage2 REAL,
    inverter_qpigs2_pv_charging_power2 REAL,
    inverter_qpiws_reserved1 INTEGER,
    inverter_qpiws_inverter_fault INTEGER,
    inverter_qpiws_bus_over INTEGER,
    inverter_qpiws_bus_under INTEGER,
    inverter_qpiws_bus_soft_fail INTEGER,
    inverter_qpiws_line_fail INTEGER,
    inverter_qpiws_opvshort INTEGER,
    inverter_qpiws_inverter_voltage_too_low INTEGER,
    inverter_qpiws_inverter_voltage_too_high INTEGER,
    inverter_qpiws_over_temperature INTEGER,
    inverter_qpiws_fan_locked INTEGER,
    inverter_qpiws_battery_voltage_high INTEGER,
    inverter_qpiws_battery_low_alarm INTEGER,
    inverter_qpiws_reserved_overcharge INTEGER,
    inverter_qpiws_battery_under_shutdown INTEGER,
    inverter_qpiws_reserved_battery_derating INTEGER,
    inverter_qpiws_over_load INTEGER,
    inverter_qpiws_eeprom_fault INTEGER,
    inverter_qpiws_inverter_over_current INTEGER,
    inverter_qpiws_inverter_soft_fail INTEGER,
    inverter_qpiws_self_test_fail INTEGER,
    inverter_qpiws_op_dv_voltage_over INTEGER,
    inverter_qpiws_bat_open INTEGER,
    inverter_qpiws_current_sensor_fail INTEGER,
    inverter_qpiws_battery_short INTEGER,
    inverter_qpiws_power_limit INTEGER,
    inverter_qpiws_pv_voltage_high_1 INTEGER,
    inverter_qpiws_mppt_overload_fault_1 INTEGER,
    inverter_qpiws_mppt_overload_warning_1 INTEGER,
    inverter_qpiws_battery_too_low_to_charge_1 INTEGER,
    inverter_qpiws_pv_voltage_high_2 INTEGER,
    inverter_qpiws_mppt_overload_fault_2 INTEGER,
    inverter_qpiws_mppt_overload_warning_2 INTEGER,
    inverter_qpiws_battery_too_low_to_charge_2 INTEGER,
    inverter_qpiws_unknown1 INTEGER,
    inverter_qpiws_unknown2 INTEGER,
    battery_bms_cell_voltage_average REAL,
    battery_bms_cell_voltage_difference_max REAL,
    battery_bms_cell_number_with_max_voltage INTEGER,
    battery_bms_cell_number_with_min_voltage INTEGER,
    battery_bms_mos_temperature REAL,
    battery_bms_battery_voltage REAL,
    battery_bms_battery_watt REAL,
    battery_bms_battery_current REAL,
    battery_bms_battery_temperature_1 REAL,
    battery_bms_battery_temperature_2 REAL,
    battery_bms_balance_current REAL,
    battery_bms_balance_state TEXT,
    battery_bms_state_of_charge INTEGER,
    battery_bms_remaining_capacity REAL,
    battery_bms_full_charge_capacity REAL,
    battery_bms_cycle_count INTEGER,
    battery_bms_total_cycle_capacity REAL,
    battery_bms_state_of_health INTEGER,
    battery_bms_precharge INTEGER,
    battery_bms_user_alarm INTEGER,
    battery_bms_runtime REAL,
    battery_bms_charge INTEGER,
    battery_bms_discharge INTEGER,
    battery_bms_user_alarm_2 INTEGER,
    battery_bms_discharge_overcurrent_protection_release_time INTEGER,
    battery_bms_discharge_short_circuit_protection_release_time INTEGER,
    battery_bms_charge_overcurrent_protection_release_time INTEGER,
    battery_bms_charge_short_circuit_protection_release_time INTEGER,
    battery_bms_undervoltage_protection_release_time INTEGER,
    battery_bms_overvoltage_protection_release_time INTEGER,
    battery_bms_heating INTEGER,
    battery_bms_reserved INTEGER,
    battery_bms_emergency_switch_time INTEGER,
    battery_bms_discharge_current_correction_factor INTEGER,
    battery_bms_charging_current_sensor_voltage REAL,
    battery_bms_discharging_current_sensor_voltage REAL,
    battery_bms_battery_voltage_correction_factor REAL,
    battery_bms_battery_voltage_2 REAL,
    battery_bms_heating_current REAL,
    battery_bms_reserved_2 INTEGER,
    battery_bms_charger_plugged INTEGER,
    battery_bms_system_runtime_ticks REAL,
    battery_bms_battery_temperature_3 REAL,
    battery_bms_battery_temperature_4 REAL,
    battery_bms_battery_temperature_5 REAL,
    battery_bms_rtc_counter INTEGER,
    battery_bms_time_enter_sleep INTEGER,
    battery_bms_parallel_current_limiting_status INTEGER,
    battery_bms_reserved_3 INTEGER,
    battery_bms_trailer TEXT,
    battery_bms_cell_0_voltage REAL,
    battery_bms_cell_1_voltage REAL,
    battery_bms_cell_2_voltage REAL,
    battery_bms_cell_3_voltage REAL,
    battery_bms_cell_4_voltage REAL,
    battery_bms_cell_5_voltage REAL,
    battery_bms_cell_6_voltage REAL,
    battery_bms_cell_7_voltage REAL,
    battery_bms_cell_8_voltage REAL,
    battery_bms_cell_9_voltage REAL,
    battery_bms_cell_10_voltage REAL,
    battery_bms_cell_11_voltage REAL,
    battery_bms_cell_12_voltage REAL,
    battery_bms_cell_13_voltage REAL,
    battery_bms_cell_14_voltage REAL,
    battery_bms_cell_15_voltage REAL,
    battery_bms_cell_0_resistance REAL,
    battery_bms_cell_1_resistance REAL,
    battery_bms_cell_2_resistance REAL,
    battery_bms_cell_3_resistance REAL,
    battery_bms_cell_4_resistance REAL,
    battery_bms_cell_5_resistance REAL,
    battery_bms_cell_6_resistance REAL,
    battery_bms_cell_7_resistance REAL,
    battery_bms_cell_8_resistance REAL,
    battery_bms_cell_9_resistance REAL,
    battery_bms_cell_10_resistance REAL,
    battery_bms_cell_11_resistance REAL,
    battery_bms_cell_12_resistance REAL,
    battery_bms_cell_13_resistance REAL,
    battery_bms_cell_14_resistance REAL,
    battery_bms_cell_15_resistance REAL,
    battery_bms_cell_status_0 INTEGER,
    battery_bms_cell_status_1 INTEGER,
    battery_bms_cell_status_2 INTEGER,
    battery_bms_cell_status_3 INTEGER,
    battery_bms_cell_status_4 INTEGER,
    battery_bms_cell_status_5 INTEGER,
    battery_bms_cell_status_6 INTEGER,
    battery_bms_cell_status_7 INTEGER,
    battery_bms_cell_status_8 INTEGER,
    battery_bms_cell_status_9 INTEGER,
    battery_bms_cell_status_10 INTEGER,
    battery_bms_cell_status_11 INTEGER,
    battery_bms_cell_status_12 INTEGER,
    battery_bms_cell_status_13 INTEGER,
    battery_bms_cell_status_14 INTEGER,
    battery_bms_cell_status_15 INTEGER,
    battery_bms_cell_status_16 INTEGER,
    battery_bms_cell_status_17 INTEGER,
    battery_bms_cell_status_18 INTEGER,
    battery_bms_cell_status_19 INTEGER,
    battery_bms_cell_status_20 INTEGER,
    battery_bms_cell_status_21 INTEGER,
    battery_bms_cell_status_22 INTEGER,
    battery_bms_cell_status_23 INTEGER,
    battery_bms_cell_status_24 INTEGER,
    battery_bms_cell_status_25 INTEGER,
    battery_bms_cell_status_26 INTEGER,
    battery_bms_cell_status_27 INTEGER,
    battery_bms_cell_status_28 INTEGER,
    battery_bms_cell_status_29 INTEGER,
    battery_bms_cell_status_30 INTEGER,
    battery_bms_cell_status_31 INTEGER,
    battery_bms_cell_wire_status_0 INTEGER,
    battery_bms_cell_wire_status_1 INTEGER,
    battery_bms_cell_wire_status_2 INTEGER,
    battery_bms_cell_wire_status_3 INTEGER,
    battery_bms_cell_wire_status_4 INTEGER,
    battery_bms_cell_wire_status_5 INTEGER,
    battery_bms_cell_wire_status_6 INTEGER,
    battery_bms_cell_wire_status_7 INTEGER,
    battery_bms_cell_wire_status_8 INTEGER,
    battery_bms_cell_wire_status_9 INTEGER,
    battery_bms_cell_wire_status_10 INTEGER,
    battery_bms_cell_wire_status_11 INTEGER,
    battery_bms_cell_wire_status_12 INTEGER,
    battery_bms_cell_wire_status_13 INTEGER,
    battery_bms_cell_wire_status_14 INTEGER,
    battery_bms_cell_wire_status_15 INTEGER,
    battery_bms_cell_wire_status_16 INTEGER,
    battery_bms_cell_wire_status_17 INTEGER,
    battery_bms_cell_wire_status_18 INTEGER,
    battery_bms_cell_wire_status_19 INTEGER,
    battery_bms_cell_wire_status_20 INTEGER,
    battery_bms_cell_wire_status_21 INTEGER,
    battery_bms_cell_wire_status_22 INTEGER,
    battery_bms_cell_wire_status_23 INTEGER,
    battery_bms_cell_wire_status_24 INTEGER,
    battery_bms_cell_wire_status_25 INTEGER,
    battery_bms_cell_wire_status_26 INTEGER,
    battery_bms_cell_wire_status_27 INTEGER,
    battery_bms_cell_wire_status_28 INTEGER,
    battery_bms_cell_wire_status_29 INTEGER,
    battery_bms_cell_wire_status_30 INTEGER,
    battery_bms_cell_wire_status_31 INTEGER,
    battery_bms_alarms_resistence_of_the_balancing_wire_too_large INTEGER,
    battery_bms_alarms_mos_overtemperature_protection INTEGER,
    battery_bms_alarms_the_number_of_cells_does_not_match_the_set_value INTEGER,
    battery_bms_alarms_current_sensor_abnormality INTEGER,
    battery_bms_alarms_single_unit_overvoltage_protection INTEGER,
    battery_bms_alarms_battery_overvoltage_protection INTEGER,
    battery_bms_alarms_charging_overcurrent_protection INTEGER,
    battery_bms_alarms_charging_short_circuit_protection INTEGER,
    battery_bms_alarms_charging_over_temperature_protection INTEGER,
    battery_bms_alarms_charging_low_temperature_protection INTEGER,
    battery_bms_alarms_internal_communication_abnormality INTEGER,
    battery_bms_alarms_single_unit_undervoltage_protection INTEGER,
    battery_bms_alarms_battery_undervoltage_protection INTEGER,
    battery_bms_alarms_discharge_overcurrent_protection INTEGER,
    battery_bms_alarms_discharge_short_circuit_protection INTEGER,
    battery_bms_alarms_discharge_over_temperature_protection INTEGER,
    battery_bms_alarms_charging_anomaly INTEGER,
    battery_bms_alarms_discharge_anomaly INTEGER,
    battery_bms_alarms_gps_disconnected INTEGER,
    battery_bms_alarms_please_change_the_authorization_password INTEGER,
    battery_bms_alarms_discharge_on_failure INTEGER,
    battery_bms_alarms_battery_over_temperature INTEGER,
    battery_bms_alarms_temperature_sensor_anomaly INTEGER,
    battery_bms_alarms_parallel_module_failure INTEGER,
    battery_bms_alarms_unknown_1 INTEGER,
    battery_bms_alarms_unknown_2 INTEGER,
    battery_bms_alarms_unknown_3 INTEGER,
    battery_bms_alarms_unknown_4 INTEGER,
    battery_bms_alarms_unknown_5 INTEGER,
    battery_bms_alarms_unknown_6 INTEGER,
    battery_bms_alarms_unknown_7 INTEGER,
    battery_bms_alarms_unknown_8 INTEGER,
    battery_bms_temperature_sensor_status_mos_temperature_sensor INTEGER,
    battery_bms_temperature_sensor_status_battery_temperature_sensor_1 INTEGER,
    battery_bms_temperature_sensor_status_battery_temperature_sensor_2 INTEGER,
    battery_bms_temperature_sensor_status_battery_temperature_sensor_3 INTEGER,
    battery_bms_temperature_sensor_status_battery_temperature_sensor_4 INTEGER,
    battery_bms_temperature_sensor_status_battery_temperature_sensor_5 INTEGER,
    battery_bms_temperature_sensor_status_unknown1 INTEGER,
    battery_bms_temperature_sensor_status_unknown2 INTEGER
  ) STRICT
`);

const insertIntoFlat = database.prepare(`
  INSERT INTO data (
    timestamp, value,
    system_uptime,
    inverter_qpigs_grid_voltage, inverter_qpigs_grid_frequency, inverter_qpigs_ac_output_voltage,
    inverter_qpigs_ac_output_frequency, inverter_qpigs_ac_output_apparent_power,
    inverter_qpigs_ac_output_active_power, inverter_qpigs_output_load_percent,
    inverter_qpigs_bus_voltage, inverter_qpigs_battery_voltage, inverter_qpigs_battery_charging_current,
    inverter_qpigs_battery_capacity, inverter_qpigs_inverter_heat_sink_temperature,
    inverter_qpigs_pv_input_current1, inverter_qpigs_pv_input_voltage1,
    inverter_qpigs_battery_voltage_from_scc1, inverter_qpigs_battery_discharge_current,
    inverter_qpigs_add_sbu_priority_version, inverter_qpigs_configuration_status,
    inverter_qpigs_scc_firmware_version, inverter_qpigs_load_status,
    inverter_qpigs_battery_voltage_to_steady_while_charging, inverter_qpigs_charging_status,
    inverter_qpigs_charging_status_scc_1, inverter_qpigs_charging_status_ac,
    inverter_qpigs_battery_voltage_from_fans_on, inverter_qpigs_eeprom_version,
    inverter_qpigs_pv_charging_power1, inverter_qpigs_flag_for_charging_to_flating_mode,
    inverter_qpigs_switch_on, inverter_qpigs_device_status_2_reserved,
    inverter_qpigs2_pv_input_current2, inverter_qpigs2_pv_input_voltage2,
    inverter_qpigs2_pv_charging_power2,
    inverter_qpiws_reserved1, inverter_qpiws_inverter_fault, inverter_qpiws_bus_over,
    inverter_qpiws_bus_under, inverter_qpiws_bus_soft_fail, inverter_qpiws_line_fail,
    inverter_qpiws_opvshort, inverter_qpiws_inverter_voltage_too_low,
    inverter_qpiws_inverter_voltage_too_high, inverter_qpiws_over_temperature,
    inverter_qpiws_fan_locked, inverter_qpiws_battery_voltage_high,
    inverter_qpiws_battery_low_alarm, inverter_qpiws_reserved_overcharge,
    inverter_qpiws_battery_under_shutdown, inverter_qpiws_reserved_battery_derating,
    inverter_qpiws_over_load, inverter_qpiws_eeprom_fault,
    inverter_qpiws_inverter_over_current, inverter_qpiws_inverter_soft_fail,
    inverter_qpiws_self_test_fail, inverter_qpiws_op_dv_voltage_over,
    inverter_qpiws_bat_open, inverter_qpiws_current_sensor_fail,
    inverter_qpiws_battery_short, inverter_qpiws_power_limit,
    inverter_qpiws_pv_voltage_high_1, inverter_qpiws_mppt_overload_fault_1,
    inverter_qpiws_mppt_overload_warning_1, inverter_qpiws_battery_too_low_to_charge_1,
    inverter_qpiws_pv_voltage_high_2, inverter_qpiws_mppt_overload_fault_2,
    inverter_qpiws_mppt_overload_warning_2, inverter_qpiws_battery_too_low_to_charge_2,
    inverter_qpiws_unknown1, inverter_qpiws_unknown2,
    battery_bms_cell_voltage_average, battery_bms_cell_voltage_difference_max,
    battery_bms_cell_number_with_max_voltage, battery_bms_cell_number_with_min_voltage,
    battery_bms_mos_temperature, battery_bms_battery_voltage, battery_bms_battery_watt,
    battery_bms_battery_current, battery_bms_battery_temperature_1, battery_bms_battery_temperature_2,
    battery_bms_balance_current, battery_bms_balance_state, battery_bms_state_of_charge,
    battery_bms_remaining_capacity, battery_bms_full_charge_capacity, battery_bms_cycle_count,
    battery_bms_total_cycle_capacity, battery_bms_state_of_health, battery_bms_precharge,
    battery_bms_user_alarm, battery_bms_runtime, battery_bms_charge, battery_bms_discharge,
    battery_bms_user_alarm_2, battery_bms_discharge_overcurrent_protection_release_time,
    battery_bms_discharge_short_circuit_protection_release_time, battery_bms_charge_overcurrent_protection_release_time,
    battery_bms_charge_short_circuit_protection_release_time, battery_bms_undervoltage_protection_release_time,
    battery_bms_overvoltage_protection_release_time, battery_bms_heating, battery_bms_reserved,
    battery_bms_emergency_switch_time, battery_bms_discharge_current_correction_factor,
    battery_bms_charging_current_sensor_voltage, battery_bms_discharging_current_sensor_voltage,
    battery_bms_battery_voltage_correction_factor, battery_bms_battery_voltage_2, battery_bms_heating_current,
    battery_bms_reserved_2, battery_bms_charger_plugged, battery_bms_system_runtime_ticks,
    battery_bms_battery_temperature_3, battery_bms_battery_temperature_4, battery_bms_battery_temperature_5,
    battery_bms_rtc_counter, battery_bms_time_enter_sleep, battery_bms_parallel_current_limiting_status,
    battery_bms_reserved_3, battery_bms_trailer,
    battery_bms_cell_0_voltage, battery_bms_cell_1_voltage, battery_bms_cell_2_voltage,
    battery_bms_cell_3_voltage, battery_bms_cell_4_voltage, battery_bms_cell_5_voltage,
    battery_bms_cell_6_voltage, battery_bms_cell_7_voltage, battery_bms_cell_8_voltage,
    battery_bms_cell_9_voltage, battery_bms_cell_10_voltage, battery_bms_cell_11_voltage,
    battery_bms_cell_12_voltage, battery_bms_cell_13_voltage, battery_bms_cell_14_voltage,
    battery_bms_cell_15_voltage,
    battery_bms_cell_0_resistance, battery_bms_cell_1_resistance, battery_bms_cell_2_resistance,
    battery_bms_cell_3_resistance, battery_bms_cell_4_resistance, battery_bms_cell_5_resistance,
    battery_bms_cell_6_resistance, battery_bms_cell_7_resistance, battery_bms_cell_8_resistance,
    battery_bms_cell_9_resistance, battery_bms_cell_10_resistance, battery_bms_cell_11_resistance,
    battery_bms_cell_12_resistance, battery_bms_cell_13_resistance, battery_bms_cell_14_resistance,
    battery_bms_cell_15_resistance,
    battery_bms_cell_status_0, battery_bms_cell_status_1, battery_bms_cell_status_2,
    battery_bms_cell_status_3, battery_bms_cell_status_4, battery_bms_cell_status_5,
    battery_bms_cell_status_6, battery_bms_cell_status_7, battery_bms_cell_status_8,
    battery_bms_cell_status_9, battery_bms_cell_status_10, battery_bms_cell_status_11,
    battery_bms_cell_status_12, battery_bms_cell_status_13, battery_bms_cell_status_14,
    battery_bms_cell_status_15, battery_bms_cell_status_16, battery_bms_cell_status_17,
    battery_bms_cell_status_18, battery_bms_cell_status_19, battery_bms_cell_status_20,
    battery_bms_cell_status_21, battery_bms_cell_status_22, battery_bms_cell_status_23,
    battery_bms_cell_status_24, battery_bms_cell_status_25, battery_bms_cell_status_26,
    battery_bms_cell_status_27, battery_bms_cell_status_28, battery_bms_cell_status_29,
    battery_bms_cell_status_30, battery_bms_cell_status_31,
    battery_bms_cell_wire_status_0, battery_bms_cell_wire_status_1, battery_bms_cell_wire_status_2,
    battery_bms_cell_wire_status_3, battery_bms_cell_wire_status_4, battery_bms_cell_wire_status_5,
    battery_bms_cell_wire_status_6, battery_bms_cell_wire_status_7, battery_bms_cell_wire_status_8,
    battery_bms_cell_wire_status_9, battery_bms_cell_wire_status_10, battery_bms_cell_wire_status_11,
    battery_bms_cell_wire_status_12, battery_bms_cell_wire_status_13, battery_bms_cell_wire_status_14,
    battery_bms_cell_wire_status_15, battery_bms_cell_wire_status_16, battery_bms_cell_wire_status_17,
    battery_bms_cell_wire_status_18, battery_bms_cell_wire_status_19, battery_bms_cell_wire_status_20,
    battery_bms_cell_wire_status_21, battery_bms_cell_wire_status_22, battery_bms_cell_wire_status_23,
    battery_bms_cell_wire_status_24, battery_bms_cell_wire_status_25, battery_bms_cell_wire_status_26,
    battery_bms_cell_wire_status_27, battery_bms_cell_wire_status_28, battery_bms_cell_wire_status_29,
    battery_bms_cell_wire_status_30, battery_bms_cell_wire_status_31,
    battery_bms_alarms_resistence_of_the_balancing_wire_too_large,
    battery_bms_alarms_mos_overtemperature_protection,
    battery_bms_alarms_the_number_of_cells_does_not_match_the_set_value,
    battery_bms_alarms_current_sensor_abnormality,
    battery_bms_alarms_single_unit_overvoltage_protection,
    battery_bms_alarms_battery_overvoltage_protection,
    battery_bms_alarms_charging_overcurrent_protection,
    battery_bms_alarms_charging_short_circuit_protection,
    battery_bms_alarms_charging_over_temperature_protection,
    battery_bms_alarms_charging_low_temperature_protection,
    battery_bms_alarms_internal_communication_abnormality,
    battery_bms_alarms_single_unit_undervoltage_protection,
    battery_bms_alarms_battery_undervoltage_protection,
    battery_bms_alarms_discharge_overcurrent_protection,
    battery_bms_alarms_discharge_short_circuit_protection,
    battery_bms_alarms_discharge_over_temperature_protection,
    battery_bms_alarms_charging_anomaly,
    battery_bms_alarms_discharge_anomaly,
    battery_bms_alarms_gps_disconnected,
    battery_bms_alarms_please_change_the_authorization_password,
    battery_bms_alarms_discharge_on_failure,
    battery_bms_alarms_battery_over_temperature,
    battery_bms_alarms_temperature_sensor_anomaly,
    battery_bms_alarms_parallel_module_failure,
    battery_bms_alarms_unknown_1, battery_bms_alarms_unknown_2, battery_bms_alarms_unknown_3,
    battery_bms_alarms_unknown_4, battery_bms_alarms_unknown_5, battery_bms_alarms_unknown_6,
    battery_bms_alarms_unknown_7, battery_bms_alarms_unknown_8,
    battery_bms_temperature_sensor_status_mos_temperature_sensor,
    battery_bms_temperature_sensor_status_battery_temperature_sensor_1,
    battery_bms_temperature_sensor_status_battery_temperature_sensor_2,
    battery_bms_temperature_sensor_status_battery_temperature_sensor_3,
    battery_bms_temperature_sensor_status_battery_temperature_sensor_4,
    battery_bms_temperature_sensor_status_battery_temperature_sensor_5,
    battery_bms_temperature_sensor_status_unknown1,
    battery_bms_temperature_sensor_status_unknown2
  ) VALUES (
    :timestamp, :value,
    :system_uptime,
    :inverter_qpigs_grid_voltage, :inverter_qpigs_grid_frequency, :inverter_qpigs_ac_output_voltage,
    :inverter_qpigs_ac_output_frequency, :inverter_qpigs_ac_output_apparent_power,
    :inverter_qpigs_ac_output_active_power, :inverter_qpigs_output_load_percent,
    :inverter_qpigs_bus_voltage, :inverter_qpigs_battery_voltage, :inverter_qpigs_battery_charging_current,
    :inverter_qpigs_battery_capacity, :inverter_qpigs_inverter_heat_sink_temperature,
    :inverter_qpigs_pv_input_current1, :inverter_qpigs_pv_input_voltage1,
    :inverter_qpigs_battery_voltage_from_scc1, :inverter_qpigs_battery_discharge_current,
    :inverter_qpigs_add_sbu_priority_version, :inverter_qpigs_configuration_status,
    :inverter_qpigs_scc_firmware_version, :inverter_qpigs_load_status,
    :inverter_qpigs_battery_voltage_to_steady_while_charging, :inverter_qpigs_charging_status,
    :inverter_qpigs_charging_status_scc_1, :inverter_qpigs_charging_status_ac,
    :inverter_qpigs_battery_voltage_from_fans_on, :inverter_qpigs_eeprom_version,
    :inverter_qpigs_pv_charging_power1, :inverter_qpigs_flag_for_charging_to_flating_mode,
    :inverter_qpigs_switch_on, :inverter_qpigs_device_status_2_reserved,
    :inverter_qpigs2_pv_input_current2, :inverter_qpigs2_pv_input_voltage2,
    :inverter_qpigs2_pv_charging_power2,
    :inverter_qpiws_reserved1, :inverter_qpiws_inverter_fault, :inverter_qpiws_bus_over,
    :inverter_qpiws_bus_under, :inverter_qpiws_bus_soft_fail, :inverter_qpiws_line_fail,
    :inverter_qpiws_opvshort, :inverter_qpiws_inverter_voltage_too_low,
    :inverter_qpiws_inverter_voltage_too_high, :inverter_qpiws_over_temperature,
    :inverter_qpiws_fan_locked, :inverter_qpiws_battery_voltage_high,
    :inverter_qpiws_battery_low_alarm, :inverter_qpiws_reserved_overcharge,
    :inverter_qpiws_battery_under_shutdown, :inverter_qpiws_reserved_battery_derating,
    :inverter_qpiws_over_load, :inverter_qpiws_eeprom_fault,
    :inverter_qpiws_inverter_over_current, :inverter_qpiws_inverter_soft_fail,
    :inverter_qpiws_self_test_fail, :inverter_qpiws_op_dv_voltage_over,
    :inverter_qpiws_bat_open, :inverter_qpiws_current_sensor_fail,
    :inverter_qpiws_battery_short, :inverter_qpiws_power_limit,
    :inverter_qpiws_pv_voltage_high_1, :inverter_qpiws_mppt_overload_fault_1,
    :inverter_qpiws_mppt_overload_warning_1, :inverter_qpiws_battery_too_low_to_charge_1,
    :inverter_qpiws_pv_voltage_high_2, :inverter_qpiws_mppt_overload_fault_2,
    :inverter_qpiws_mppt_overload_warning_2, :inverter_qpiws_battery_too_low_to_charge_2,
    :inverter_qpiws_unknown1, :inverter_qpiws_unknown2,
    :battery_bms_cell_voltage_average, :battery_bms_cell_voltage_difference_max,
    :battery_bms_cell_number_with_max_voltage, :battery_bms_cell_number_with_min_voltage,
    :battery_bms_mos_temperature, :battery_bms_battery_voltage, :battery_bms_battery_watt,
    :battery_bms_battery_current, :battery_bms_battery_temperature_1, :battery_bms_battery_temperature_2,
    :battery_bms_balance_current, :battery_bms_balance_state, :battery_bms_state_of_charge,
    :battery_bms_remaining_capacity, :battery_bms_full_charge_capacity, :battery_bms_cycle_count,
    :battery_bms_total_cycle_capacity, :battery_bms_state_of_health, :battery_bms_precharge,
    :battery_bms_user_alarm, :battery_bms_runtime, :battery_bms_charge, :battery_bms_discharge,
    :battery_bms_user_alarm_2, :battery_bms_discharge_overcurrent_protection_release_time,
    :battery_bms_discharge_short_circuit_protection_release_time, :battery_bms_charge_overcurrent_protection_release_time,
    :battery_bms_charge_short_circuit_protection_release_time, :battery_bms_undervoltage_protection_release_time,
    :battery_bms_overvoltage_protection_release_time, :battery_bms_heating, :battery_bms_reserved,
    :battery_bms_emergency_switch_time, :battery_bms_discharge_current_correction_factor,
    :battery_bms_charging_current_sensor_voltage, :battery_bms_discharging_current_sensor_voltage,
    :battery_bms_battery_voltage_correction_factor, :battery_bms_battery_voltage_2, :battery_bms_heating_current,
    :battery_bms_reserved_2, :battery_bms_charger_plugged, :battery_bms_system_runtime_ticks,
    :battery_bms_battery_temperature_3, :battery_bms_battery_temperature_4, :battery_bms_battery_temperature_5,
    :battery_bms_rtc_counter, :battery_bms_time_enter_sleep, :battery_bms_parallel_current_limiting_status,
    :battery_bms_reserved_3, :battery_bms_trailer,
    :battery_bms_cell_0_voltage, :battery_bms_cell_1_voltage, :battery_bms_cell_2_voltage,
    :battery_bms_cell_3_voltage, :battery_bms_cell_4_voltage, :battery_bms_cell_5_voltage,
    :battery_bms_cell_6_voltage, :battery_bms_cell_7_voltage, :battery_bms_cell_8_voltage,
    :battery_bms_cell_9_voltage, :battery_bms_cell_10_voltage, :battery_bms_cell_11_voltage,
    :battery_bms_cell_12_voltage, :battery_bms_cell_13_voltage, :battery_bms_cell_14_voltage,
    :battery_bms_cell_15_voltage,
    :battery_bms_cell_0_resistance, :battery_bms_cell_1_resistance, :battery_bms_cell_2_resistance,
    :battery_bms_cell_3_resistance, :battery_bms_cell_4_resistance, :battery_bms_cell_5_resistance,
    :battery_bms_cell_6_resistance, :battery_bms_cell_7_resistance, :battery_bms_cell_8_resistance,
    :battery_bms_cell_9_resistance, :battery_bms_cell_10_resistance, :battery_bms_cell_11_resistance,
    :battery_bms_cell_12_resistance, :battery_bms_cell_13_resistance, :battery_bms_cell_14_resistance,
    :battery_bms_cell_15_resistance,
    :battery_bms_cell_status_0, :battery_bms_cell_status_1, :battery_bms_cell_status_2,
    :battery_bms_cell_status_3, :battery_bms_cell_status_4, :battery_bms_cell_status_5,
    :battery_bms_cell_status_6, :battery_bms_cell_status_7, :battery_bms_cell_status_8,
    :battery_bms_cell_status_9, :battery_bms_cell_status_10, :battery_bms_cell_status_11,
    :battery_bms_cell_status_12, :battery_bms_cell_status_13, :battery_bms_cell_status_14,
    :battery_bms_cell_status_15, :battery_bms_cell_status_16, :battery_bms_cell_status_17,
    :battery_bms_cell_status_18, :battery_bms_cell_status_19, :battery_bms_cell_status_20,
    :battery_bms_cell_status_21, :battery_bms_cell_status_22, :battery_bms_cell_status_23,
    :battery_bms_cell_status_24, :battery_bms_cell_status_25, :battery_bms_cell_status_26,
    :battery_bms_cell_status_27, :battery_bms_cell_status_28, :battery_bms_cell_status_29,
    :battery_bms_cell_status_30, :battery_bms_cell_status_31,
    :battery_bms_cell_wire_status_0, :battery_bms_cell_wire_status_1, :battery_bms_cell_wire_status_2,
    :battery_bms_cell_wire_status_3, :battery_bms_cell_wire_status_4, :battery_bms_cell_wire_status_5,
    :battery_bms_cell_wire_status_6, :battery_bms_cell_wire_status_7, :battery_bms_cell_wire_status_8,
    :battery_bms_cell_wire_status_9, :battery_bms_cell_wire_status_10, :battery_bms_cell_wire_status_11,
    :battery_bms_cell_wire_status_12, :battery_bms_cell_wire_status_13, :battery_bms_cell_wire_status_14,
    :battery_bms_cell_wire_status_15, :battery_bms_cell_wire_status_16, :battery_bms_cell_wire_status_17,
    :battery_bms_cell_wire_status_18, :battery_bms_cell_wire_status_19, :battery_bms_cell_wire_status_20,
    :battery_bms_cell_wire_status_21, :battery_bms_cell_wire_status_22, :battery_bms_cell_wire_status_23,
    :battery_bms_cell_wire_status_24, :battery_bms_cell_wire_status_25, :battery_bms_cell_wire_status_26,
    :battery_bms_cell_wire_status_27, :battery_bms_cell_wire_status_28, :battery_bms_cell_wire_status_29,
    :battery_bms_cell_wire_status_30, :battery_bms_cell_wire_status_31,
    :battery_bms_alarms_resistence_of_the_balancing_wire_too_large,
    :battery_bms_alarms_mos_overtemperature_protection,
    :battery_bms_alarms_the_number_of_cells_does_not_match_the_set_value,
    :battery_bms_alarms_current_sensor_abnormality,
    :battery_bms_alarms_single_unit_overvoltage_protection,
    :battery_bms_alarms_battery_overvoltage_protection,
    :battery_bms_alarms_charging_overcurrent_protection,
    :battery_bms_alarms_charging_short_circuit_protection,
    :battery_bms_alarms_charging_over_temperature_protection,
    :battery_bms_alarms_charging_low_temperature_protection,
    :battery_bms_alarms_internal_communication_abnormality,
    :battery_bms_alarms_single_unit_undervoltage_protection,
    :battery_bms_alarms_battery_undervoltage_protection,
    :battery_bms_alarms_discharge_overcurrent_protection,
    :battery_bms_alarms_discharge_short_circuit_protection,
    :battery_bms_alarms_discharge_over_temperature_protection,
    :battery_bms_alarms_charging_anomaly,
    :battery_bms_alarms_discharge_anomaly,
    :battery_bms_alarms_gps_disconnected,
    :battery_bms_alarms_please_change_the_authorization_password,
    :battery_bms_alarms_discharge_on_failure,
    :battery_bms_alarms_battery_over_temperature,
    :battery_bms_alarms_temperature_sensor_anomaly,
    :battery_bms_alarms_parallel_module_failure,
    :battery_bms_alarms_unknown_1, :battery_bms_alarms_unknown_2, :battery_bms_alarms_unknown_3,
    :battery_bms_alarms_unknown_4, :battery_bms_alarms_unknown_5, :battery_bms_alarms_unknown_6,
    :battery_bms_alarms_unknown_7, :battery_bms_alarms_unknown_8,
    :battery_bms_temperature_sensor_status_mos_temperature_sensor,
    :battery_bms_temperature_sensor_status_battery_temperature_sensor_1,
    :battery_bms_temperature_sensor_status_battery_temperature_sensor_2,
    :battery_bms_temperature_sensor_status_battery_temperature_sensor_3,
    :battery_bms_temperature_sensor_status_battery_temperature_sensor_4,
    :battery_bms_temperature_sensor_status_battery_temperature_sensor_5,
    :battery_bms_temperature_sensor_status_unknown1,
    :battery_bms_temperature_sensor_status_unknown2
  )
`);

// https://r1ch.net/blog/node-v20-aggregateeerror-etimedout-happy-eyeballs
// https://github.com/nodejs/node/issues/54359
//net.setDefaultAutoSelectFamilyAttemptTimeout(1000);

const binding = autoDetect();

const request = async (path: string, command: typeof commands[0], signal: AbortSignal) => {
	const port = await binding.open({path, baudRate: 2400, dataBits: 8, stopBits: 1, parity: "none"});
	const finished = new AbortController();
	try {
		signal.throwIfAborted();
		const [, , values] = await Promise.all([
			new Promise((res, rej) => {
				signal.addEventListener("abort", (reason) => {
					port.close().catch((e) => rej(e));
					rej(reason);
				}, {once: true, signal: finished.signal});
				finished.signal.addEventListener("abort", (r) => res(r), {once: true, signal});
			}),
			(async () => {
				const commandBytes = new TextEncoder().encode(command.command);
				const fullCommandBytes = new Uint8Array([...commandBytes, ...modifiedCrc(commandBytes), 13]);
				try {
					await port.write(Buffer.from(fullCommandBytes, fullCommandBytes.byteLength, fullCommandBytes.byteLength));
				}catch(e) {
					if (!e.canceled) {
						throw e;
					}
				}
			})(),
			(async () => {
				let buffer = new Uint8Array(0);
				while(true) {
					const readBytes = await (async () => {
						try {
							const {buffer, bytesRead} = await port.read(Buffer.alloc(8), 0, 8);
							return buffer.subarray(0, bytesRead);
						}catch(e) {
							if (!e.canceled) {
								throw e;
							}
						}
					})();
					if (readBytes === undefined) {
						break;
					}
					buffer = new Uint8Array([...buffer, ...readBytes]);
					//console.log([...buffer].map((a) => a >= 32 && a <= 126 ? String.fromCharCode(a) : "\\x" + a.toString(16).padStart(2, "0")).join(""));

					const checkCrc = (numbers: Uint8Array) => {
						const calculatedCrc = modifiedCrc(new Uint8Array(numbers.slice(0, -2)));
						const actualCrc = numbers.slice(-2);
						return calculatedCrc.length === actualCrc.length && calculatedCrc.every((v, i) => v === actualCrc[i]);
					};

					const {skipTo, result} = buffer.reduce(({skipTo, result}, e, i) => {
						if (i < skipTo || result !== undefined) {
							return {skipTo, result};
						}else {
							if (e === 40) {
								if (buffer[i + command.length + 3] !== 13) {
									return [];
								}
								const bytes = new Uint8Array(buffer.slice(i, i + command.length + 4));
								const crcOk = checkCrc(bytes.subarray(0, -1));
								if (!crcOk) {
									console.log(`CRC not correct. Got: ${[...bytes.subarray(-3, -1)].map((a) => a.toString(16).padStart(2, "0")).join("")}, but expected: ${[...modifiedCrc(new Uint8Array(bytes.slice(0, -3)))].map((a) => a.toString(16).padStart(2, "0")).join("")}. Message: ${[...bytes].map((a) => a.toString(16).padStart(2, "0")).join("")}`);
									return [];
								}
								//console.log(`CRC correct. Got: ${[...bytes.subarray(-3, -1)].map((a) => a.toString(16).padStart(2, "0")).join("")}. Message: ${[...bytes].map((a) => a.toString(16).padStart(2, "0")).join("")}`);
								const parsed = command.parse(new TextDecoder().decode(bytes.subarray(1, -3)));
								if (!parsed) {
								return {skipTo, result};
								}else {
									return {
										skipTo: i + command.length + 3,
										result: {
											from: i,
											to: i + command.length + 3,
											command: command.command,
											values: parsed,
											bytes,
										}
									};
								}
							}else {
								return {skipTo, result};
							}
						}
					}, {skipTo: 0, result: undefined});

					buffer = new Uint8Array(buffer.subarray(skipTo));

					if (result) {
						finished.abort();
						return result.values;
					}
				}
			})(),
		]);
		return values;
	}finally {
		if (port.isOpen) {
			await port.close();
		}
	}
};

const readBms = async (path: string, signal: AbortSignal) => {
	const port = await binding.open({path, baudRate: 115200, dataBits: 8, stopBits: 1, parity: "none"});
	const finished = new AbortController();
	try {
		signal.throwIfAborted();
		const [, values] = await Promise.all([
			new Promise((res, rej) => {
				signal.addEventListener("abort", (reason) => {
					port.close().catch((e) => rej(e));
					rej(reason);
				}, {once: true, signal: finished.signal});
				finished.signal.addEventListener("abort", (r) => res(r), {once: true, signal});
			}),
			(async () => {
				let buffer = new Uint8Array(0);
				while(true) {
					const readBytes = await (async () => {
						try {
							const {buffer, bytesRead} = await port.read(Buffer.alloc(560), 0, 560);
							return buffer.subarray(0, bytesRead);
						}catch(e) {
							if (!e.canceled) {
								throw e;
							}
						}
					})();
					if (readBytes === undefined) {
						break;
					}
					buffer = new Uint8Array([...buffer, ...readBytes]);
					//console.log([...buffer].map((a) => a.toString(16).padStart(2, "0")).join(""));

					const {skipTo, result} = buffer.reduce(({skipTo, result}, e, i) => {
						if (i < skipTo || result !== undefined) {
							return {skipTo, result};
						}else {
							if (e === 85) { // hex 55
								const bytes = new Uint8Array(buffer.slice(i, i + bms.length));
								const parsed = bms.parse(bytes);
								if (!parsed) {
									return {skipTo, result};
								}else {
									return {
										skipTo: i + bms.length + 1,
										result: {
											from: i,
											to: i + bms.length,
											values: parsed,
											bytes,
										}
									};
								}
							}else {
								return {skipTo, result};
							}
						}
					}, {skipTo: 0, result: undefined});
					buffer = new Uint8Array(buffer.subarray(skipTo));

					if (result) {
						finished.abort();
						return result.values;
					}
				}
			})(),
		]);
		return values;
	}finally {
		if (port.isOpen) {
			await port.close();
		}
	}
};

const getAllUSBPaths = async () => {
	const serialList = await binding.list();
	console.log(serialList)
	return serialList.map(({path}) => path).filter((path) => path.includes("USB"));
};

const detectBmsPath = async (paths: string[]) => {
	console.log(`Trying to find bms path: ${paths}`);
	const results = await Promise.allSettled(paths.map(async (path) => {
		await readBms(path, AbortSignal.timeout(10000));
		return path;
	}));
	console.log(results);
	const foundPath = results.filter(({status}) => status === "fulfilled").map(({value}) => value);
	return foundPath[0];

}

const detectInverterPath = async (paths: string[]) => {
	console.log(`Trying to find inverter path: ${paths}`);
	const results = await Promise.allSettled(paths.map(async (path) => {
		await request(path, commands.find(({command}) => command === "QPIGS")!, AbortSignal.timeout(2000));
		return path;
	}));
	console.log(results);
	const foundPath = results.filter(({status}) => status === "fulfilled").map(({value}) => value);
	return foundPath[0];
}

const allUSBPaths = await getAllUSBPaths();
const bmsPath = await detectBmsPath(allUSBPaths);

console.log(`BMS path: ${bmsPath}`);

const inverterPath = await detectInverterPath(allUSBPaths.filter((p) => p !== bmsPath));

console.log(`Inverter path: ${inverterPath}`);

const readCredential = async (credentialName: string, envVarName: string) => {
	if (process.env.CREDENTIALS_DIRECTORY) {
		return await fs.readFile(path.join(process.env.CREDENTIALS_DIRECTORY, credentialName), "utf8");
	}else {
		return process.env[envVarName];
	}
}

const thingspeakKey = await readCredential("thingspeak-key", "THINGSPEAK_KEY");

const [inverterValuesRes, bmsValuesRes, systemRes] = await Promise.allSettled([
	(async () => {
		if (!inverterPath) {
			throw new Error("Inverter path not found");
		}
		const qpigs = await request(inverterPath, commands.find(({command}) => command === "QPIGS")!, AbortSignal.timeout(2000));
		console.log(qpigs);
		const qpigs2 = await request(inverterPath, commands.find(({command}) => command === "QPIGS2")!, AbortSignal.timeout(2000));
		console.log(qpigs2);
		const qpiws = await request(inverterPath, commands.find(({command}) => command === "QPIWS")!, AbortSignal.timeout(2000));
		console.log(qpiws);
		return {qpigs, qpigs2, qpiws};
	})(),
	(async () => {
		if (!bms) {
			throw new Error("Bms path not found");
		}
		return await readBms(bmsPath, AbortSignal.timeout(10000));
	})(),
	(async () => {
		const uptimeRaw = await fs.readFile("/proc/uptime", "utf8");
		return {
			uptime: Math.round(Number(uptimeRaw.split(" ")[0])),
		};
	})(),
]);

console.log(bmsValuesRes);
console.log(inverterValuesRes);
console.log(systemRes);

// check charge/discharge from inverter
if(inverterValuesRes.status === "fulfilled") {
	const {qpigs} = inverterValuesRes.value;
	if (qpigs.battery_charging_current !== 0 && qpigs.battery_discharge_current !== 0) {
		throw new Error(`Both battery charging current and battery discharge current are non-null! qpigs.battery_charging_current = ${qpigs.battery_charging_current}, qpigs.battery_discharge_current = ${qpigs.battery_discharge_current}`);
	}
}

const timestamp = new Date().getTime();
const inverterData = inverterValuesRes.status === "fulfilled" ? inverterValuesRes.value : null;
const batteryData = bmsValuesRes.status === "fulfilled" ? bmsValuesRes.value : null;
const systemData = systemRes.status === "fulfilled" ? systemRes.value : null;

const boolToNumber = (b) => {
	if (b === undefined || b === null) {
		return null;
	}else {
		return b ? 1 : 0;
	}
}

insertIntoFlat.run({
  timestamp,
  //value: JSON.stringify({inverter: inverterData, battery: batteryData ? {bms: batteryData} : null, system: systemData}),
  value: null,
  system_uptime: systemData?.uptime,
  inverter_qpigs_grid_voltage: inverterData?.qpigs?.grid_voltage ?? null,
  inverter_qpigs_grid_frequency: inverterData?.qpigs?.grid_frequency ?? null,
  inverter_qpigs_ac_output_voltage: inverterData?.qpigs?.ac_output_voltage ?? null,
  inverter_qpigs_ac_output_frequency: inverterData?.qpigs?.ac_output_frequency ?? null,
  inverter_qpigs_ac_output_apparent_power: inverterData?.qpigs?.ac_output_apparent_power ?? null,
  inverter_qpigs_ac_output_active_power: inverterData?.qpigs?.ac_output_active_power ?? null,
  inverter_qpigs_output_load_percent: inverterData?.qpigs?.output_load_percent ?? null,
  inverter_qpigs_bus_voltage: inverterData?.qpigs?.bus_voltage ?? null,
  inverter_qpigs_battery_voltage: inverterData?.qpigs?.battery_voltage ?? null,
  inverter_qpigs_battery_charging_current: inverterData?.qpigs?.battery_charging_current ?? null,
  inverter_qpigs_battery_capacity: inverterData?.qpigs?.battery_capacity ?? null,
  inverter_qpigs_inverter_heat_sink_temperature: inverterData?.qpigs?.inverter_heat_sink_temperature ?? null,
  inverter_qpigs_pv_input_current1: inverterData?.qpigs?.pv_input_current1 ?? null,
  inverter_qpigs_pv_input_voltage1: inverterData?.qpigs?.pv_input_voltage1 ?? null,
  inverter_qpigs_battery_voltage_from_scc1: inverterData?.qpigs?.battery_voltage_from_scc1 ?? null,
  inverter_qpigs_battery_discharge_current: inverterData?.qpigs?.battery_discharge_current ?? null,
  inverter_qpigs_add_sbu_priority_version: boolToNumber(inverterData?.qpigs?.add_sbu_priority_version),
  inverter_qpigs_configuration_status: boolToNumber(inverterData?.qpigs?.configuration_status),
  inverter_qpigs_scc_firmware_version: boolToNumber(inverterData?.qpigs?.scc_firmware_version),
  inverter_qpigs_load_status: boolToNumber(inverterData?.qpigs?.load_status),
  inverter_qpigs_battery_voltage_to_steady_while_charging: boolToNumber(inverterData?.qpigs?.battery_voltage_to_steady_while_charging),
  inverter_qpigs_charging_status: boolToNumber(inverterData?.qpigs?.charging_status),
  inverter_qpigs_charging_status_scc_1: boolToNumber(inverterData?.qpigs?.charging_status_scc_1),
  inverter_qpigs_charging_status_ac: boolToNumber(inverterData?.qpigs?.charging_status_ac),
  inverter_qpigs_battery_voltage_from_fans_on: inverterData?.qpigs?.battery_voltage_from_fans_on ?? null,
  inverter_qpigs_eeprom_version: inverterData?.qpigs?.eeprom_version ?? null,
  inverter_qpigs_pv_charging_power1: inverterData?.qpigs?.pv_charging_power1 ?? null,
  inverter_qpigs_flag_for_charging_to_flating_mode: boolToNumber(inverterData?.qpigs?.flag_for_charging_to_flating_mode),
  inverter_qpigs_switch_on: boolToNumber(inverterData?.qpigs?.switch_on),
  inverter_qpigs_device_status_2_reserved: boolToNumber(inverterData?.qpigs?.device_status_2_reserved),
  inverter_qpigs2_pv_input_current2: inverterData?.qpigs2?.pv_input_current2 ?? null,
  inverter_qpigs2_pv_input_voltage2: inverterData?.qpigs2?.pv_input_voltage2 ?? null,
  inverter_qpigs2_pv_charging_power2: inverterData?.qpigs2?.pv_charging_power2 ?? null,
  inverter_qpiws_reserved1: boolToNumber(inverterData?.qpiws?.reserved1),
  inverter_qpiws_inverter_fault: boolToNumber(inverterData?.qpiws?.inverter_fault),
  inverter_qpiws_bus_over: boolToNumber(inverterData?.qpiws?.bus_over),
  inverter_qpiws_bus_under: boolToNumber(inverterData?.qpiws?.bus_under),
  inverter_qpiws_bus_soft_fail: boolToNumber(inverterData?.qpiws?.bus_soft_fail),
  inverter_qpiws_line_fail: boolToNumber(inverterData?.qpiws?.line_fail),
  inverter_qpiws_opvshort: boolToNumber(inverterData?.qpiws?.opvshort),
  inverter_qpiws_inverter_voltage_too_low: boolToNumber(inverterData?.qpiws?.inverter_voltage_too_low),
  inverter_qpiws_inverter_voltage_too_high: boolToNumber(inverterData?.qpiws?.inverter_voltage_too_high),
  inverter_qpiws_over_temperature: boolToNumber(inverterData?.qpiws?.over_temperature),
  inverter_qpiws_fan_locked: boolToNumber(inverterData?.qpiws?.fan_locked),
  inverter_qpiws_battery_voltage_high: boolToNumber(inverterData?.qpiws?.battery_voltage_high),
  inverter_qpiws_battery_low_alarm: boolToNumber(inverterData?.qpiws?.battery_low_alarm),
  inverter_qpiws_reserved_overcharge: boolToNumber(inverterData?.qpiws?.reserved_overcharge),
  inverter_qpiws_battery_under_shutdown: boolToNumber(inverterData?.qpiws?.battery_under_shutdown),
  inverter_qpiws_reserved_battery_derating: boolToNumber(inverterData?.qpiws?.reserved_battery_derating),
  inverter_qpiws_over_load: boolToNumber(inverterData?.qpiws?.over_load),
  inverter_qpiws_eeprom_fault: boolToNumber(inverterData?.qpiws?.eeprom_fault),
  inverter_qpiws_inverter_over_current: boolToNumber(inverterData?.qpiws?.inverter_over_current),
  inverter_qpiws_inverter_soft_fail: boolToNumber(inverterData?.qpiws?.inverter_soft_fail),
  inverter_qpiws_self_test_fail: boolToNumber(inverterData?.qpiws?.self_test_fail),
  inverter_qpiws_op_dv_voltage_over: boolToNumber(inverterData?.qpiws?.op_dv_voltage_over),
  inverter_qpiws_bat_open: boolToNumber(inverterData?.qpiws?.bat_open),
  inverter_qpiws_current_sensor_fail: boolToNumber(inverterData?.qpiws?.current_sensor_fail),
  inverter_qpiws_battery_short: boolToNumber(inverterData?.qpiws?.battery_short),
  inverter_qpiws_power_limit: boolToNumber(inverterData?.qpiws?.power_limit),
  inverter_qpiws_pv_voltage_high_1: boolToNumber(inverterData?.qpiws?.pv_voltage_high_1),
  inverter_qpiws_mppt_overload_fault_1: boolToNumber(inverterData?.qpiws?.mppt_overload_fault_1),
  inverter_qpiws_mppt_overload_warning_1: boolToNumber(inverterData?.qpiws?.mppt_overload_warning_1),
  inverter_qpiws_battery_too_low_to_charge_1: boolToNumber(inverterData?.qpiws?.battery_too_low_to_charge_1),
  inverter_qpiws_pv_voltage_high_2: boolToNumber(inverterData?.qpiws?.pv_voltage_high_2),
  inverter_qpiws_mppt_overload_fault_2: boolToNumber(inverterData?.qpiws?.mppt_overload_fault_2),
  inverter_qpiws_mppt_overload_warning_2: boolToNumber(inverterData?.qpiws?.mppt_overload_warning_2),
  inverter_qpiws_battery_too_low_to_charge_2: boolToNumber(inverterData?.qpiws?.battery_too_low_to_charge_2),
  inverter_qpiws_unknown1: boolToNumber(inverterData?.qpiws?.unknown1),
  inverter_qpiws_unknown2: boolToNumber(inverterData?.qpiws?.unknown2),
  battery_bms_cell_voltage_average: batteryData?.cell_voltage_average,
  battery_bms_cell_voltage_difference_max: batteryData?.cell_voltage_difference_max,
  battery_bms_cell_number_with_max_voltage: batteryData?.cell_number_with_max_voltage ?? null,
  battery_bms_cell_number_with_min_voltage: batteryData?.cell_number_with_min_voltage ?? null,
  battery_bms_mos_temperature: batteryData?.mos_temperature ?? null,
  battery_bms_battery_voltage: batteryData?.battery_voltage ?? null,
  battery_bms_battery_watt: batteryData?.battery_watt ?? null,
  battery_bms_battery_current: batteryData?.battery_current ?? null,
  battery_bms_battery_temperature_1: batteryData?.battery_temperature_1 ?? null,
  battery_bms_battery_temperature_2: batteryData?.battery_temperature_2 ?? null,
  battery_bms_balance_current: batteryData?.balance_current ?? null,
  battery_bms_balance_state: batteryData?.balance_state ?? null,
  battery_bms_state_of_charge: batteryData?.state_of_charge ?? null,
  battery_bms_remaining_capacity: batteryData?.remaining_capacity ?? null,
  battery_bms_full_charge_capacity: batteryData?.full_charge_capacity ?? null,
  battery_bms_cycle_count: batteryData?.cycle_count ?? null,
  battery_bms_total_cycle_capacity: batteryData?.total_cycle_capacity ?? null,
  battery_bms_state_of_health: batteryData?.state_of_health ?? null,
  battery_bms_precharge: batteryData?.precharge ?? null,
  battery_bms_user_alarm: batteryData?.user_alarm ?? null,
  battery_bms_runtime: batteryData?.runtime ?? null,
  battery_bms_charge: batteryData?.charge ?? null,
  battery_bms_discharge: batteryData?.discharge ?? null,
  battery_bms_user_alarm_2: batteryData?.user_alarm_2 ?? null,
  battery_bms_discharge_overcurrent_protection_release_time: batteryData?.discharge_overcurrent_protection_release_time ?? null,
  battery_bms_discharge_short_circuit_protection_release_time: batteryData?.discharge_short_circuit_protection_release_time ?? null,
  battery_bms_charge_overcurrent_protection_release_time: batteryData?.charge_overcurrent_protection_release_time ?? null,
  battery_bms_charge_short_circuit_protection_release_time: batteryData?.charge_short_circuit_protection_release_time ?? null,
  battery_bms_undervoltage_protection_release_time: batteryData?.undervoltage_protection_release_time ?? null,
  battery_bms_overvoltage_protection_release_time: batteryData?.overvoltage_protection_release_time ?? null,
  battery_bms_heating: batteryData?.heating ?? null,
  battery_bms_reserved: batteryData?.reserved ?? null,
  battery_bms_emergency_switch_time: batteryData?.emergency_switch_time ?? null,
  battery_bms_discharge_current_correction_factor: batteryData?.discharge_current_correction_factor ?? null,
  battery_bms_charging_current_sensor_voltage: batteryData?.charging_current_sensor_voltage ?? null,
  battery_bms_discharging_current_sensor_voltage: batteryData?.discharging_current_sensor_voltage ?? null,
  battery_bms_battery_voltage_correction_factor: batteryData?.battery_voltage_correction_factor ?? null,
  battery_bms_battery_voltage_2: batteryData?.battery_voltage_2 ?? null,
  battery_bms_heating_current: batteryData?.heating_current ?? null,
  battery_bms_reserved_2: batteryData?.reserved_2 ?? null,
  battery_bms_charger_plugged: batteryData?.charger_plugged ?? null,
  battery_bms_system_runtime_ticks: batteryData?.system_runtime_ticks ?? null,
  battery_bms_battery_temperature_3: batteryData?.battery_temperature_3 ?? null,
  battery_bms_battery_temperature_4: batteryData?.battery_temperature_4 ?? null,
  battery_bms_battery_temperature_5: batteryData?.battery_temperature_5 ?? null,
  battery_bms_rtc_counter: batteryData?.rtc_counter ?? null,
  battery_bms_time_enter_sleep: batteryData?.time_enter_sleep ?? null,
  battery_bms_parallel_current_limiting_status: batteryData?.parallel_current_limiting_status ?? null,
  battery_bms_reserved_3: batteryData?.reserved_3 ?? null,
  battery_bms_trailer: batteryData?.trailer ?? null,
  battery_bms_cell_0_voltage: batteryData?.cell_0_voltage ?? null,
  battery_bms_cell_1_voltage: batteryData?.cell_1_voltage ?? null,
  battery_bms_cell_2_voltage: batteryData?.cell_2_voltage ?? null,
  battery_bms_cell_3_voltage: batteryData?.cell_3_voltage ?? null,
  battery_bms_cell_4_voltage: batteryData?.cell_4_voltage ?? null,
  battery_bms_cell_5_voltage: batteryData?.cell_5_voltage ?? null,
  battery_bms_cell_6_voltage: batteryData?.cell_6_voltage ?? null,
  battery_bms_cell_7_voltage: batteryData?.cell_7_voltage ?? null,
  battery_bms_cell_8_voltage: batteryData?.cell_8_voltage ?? null,
  battery_bms_cell_9_voltage: batteryData?.cell_9_voltage ?? null,
  battery_bms_cell_10_voltage: batteryData?.cell_10_voltage ?? null,
  battery_bms_cell_11_voltage: batteryData?.cell_11_voltage ?? null,
  battery_bms_cell_12_voltage: batteryData?.cell_12_voltage ?? null,
  battery_bms_cell_13_voltage: batteryData?.cell_13_voltage ?? null,
  battery_bms_cell_14_voltage: batteryData?.cell_14_voltage ?? null,
  battery_bms_cell_15_voltage: batteryData?.cell_15_voltage ?? null,
  battery_bms_cell_0_resistance: batteryData?.cell_0_resistance ?? null,
  battery_bms_cell_1_resistance: batteryData?.cell_1_resistance ?? null,
  battery_bms_cell_2_resistance: batteryData?.cell_2_resistance ?? null,
  battery_bms_cell_3_resistance: batteryData?.cell_3_resistance ?? null,
  battery_bms_cell_4_resistance: batteryData?.cell_4_resistance ?? null,
  battery_bms_cell_5_resistance: batteryData?.cell_5_resistance ?? null,
  battery_bms_cell_6_resistance: batteryData?.cell_6_resistance ?? null,
  battery_bms_cell_7_resistance: batteryData?.cell_7_resistance ?? null,
  battery_bms_cell_8_resistance: batteryData?.cell_8_resistance ?? null,
  battery_bms_cell_9_resistance: batteryData?.cell_9_resistance ?? null,
  battery_bms_cell_10_resistance: batteryData?.cell_10_resistance ?? null,
  battery_bms_cell_11_resistance: batteryData?.cell_11_resistance ?? null,
  battery_bms_cell_12_resistance: batteryData?.cell_12_resistance ?? null,
  battery_bms_cell_13_resistance: batteryData?.cell_13_resistance ?? null,
  battery_bms_cell_14_resistance: batteryData?.cell_14_resistance ?? null,
  battery_bms_cell_15_resistance: batteryData?.cell_15_resistance ?? null,
  battery_bms_cell_status_0: boolToNumber(batteryData?.cell_status?.[0]),
  battery_bms_cell_status_1: boolToNumber(batteryData?.cell_status?.[1]),
  battery_bms_cell_status_2: boolToNumber(batteryData?.cell_status?.[2]),
  battery_bms_cell_status_3: boolToNumber(batteryData?.cell_status?.[3]),
  battery_bms_cell_status_4: boolToNumber(batteryData?.cell_status?.[4]),
  battery_bms_cell_status_5: boolToNumber(batteryData?.cell_status?.[5]),
  battery_bms_cell_status_6: boolToNumber(batteryData?.cell_status?.[6]),
  battery_bms_cell_status_7: boolToNumber(batteryData?.cell_status?.[7]),
  battery_bms_cell_status_8: boolToNumber(batteryData?.cell_status?.[8]),
  battery_bms_cell_status_9: boolToNumber(batteryData?.cell_status?.[9]),
  battery_bms_cell_status_10: boolToNumber(batteryData?.cell_status?.[10]),
  battery_bms_cell_status_11: boolToNumber(batteryData?.cell_status?.[11]),
  battery_bms_cell_status_12: boolToNumber(batteryData?.cell_status?.[12]),
  battery_bms_cell_status_13: boolToNumber(batteryData?.cell_status?.[13]),
  battery_bms_cell_status_14: boolToNumber(batteryData?.cell_status?.[14]),
  battery_bms_cell_status_15: boolToNumber(batteryData?.cell_status?.[15]),
  battery_bms_cell_status_16: boolToNumber(batteryData?.cell_status?.[16]),
  battery_bms_cell_status_17: boolToNumber(batteryData?.cell_status?.[17]),
  battery_bms_cell_status_18: boolToNumber(batteryData?.cell_status?.[18]),
  battery_bms_cell_status_19: boolToNumber(batteryData?.cell_status?.[19]),
  battery_bms_cell_status_20: boolToNumber(batteryData?.cell_status?.[20]),
  battery_bms_cell_status_21: boolToNumber(batteryData?.cell_status?.[21]),
  battery_bms_cell_status_22: boolToNumber(batteryData?.cell_status?.[22]),
  battery_bms_cell_status_23: boolToNumber(batteryData?.cell_status?.[23]),
  battery_bms_cell_status_24: boolToNumber(batteryData?.cell_status?.[24]),
  battery_bms_cell_status_25: boolToNumber(batteryData?.cell_status?.[25]),
  battery_bms_cell_status_26: boolToNumber(batteryData?.cell_status?.[26]),
  battery_bms_cell_status_27: boolToNumber(batteryData?.cell_status?.[27]),
  battery_bms_cell_status_28: boolToNumber(batteryData?.cell_status?.[28]),
  battery_bms_cell_status_29: boolToNumber(batteryData?.cell_status?.[29]),
  battery_bms_cell_status_30: boolToNumber(batteryData?.cell_status?.[30]),
  battery_bms_cell_status_31: boolToNumber(batteryData?.cell_status?.[31]),
  battery_bms_cell_wire_status_0: boolToNumber(batteryData?.cell_wire_resistance_status?.[0]),
  battery_bms_cell_wire_status_1: boolToNumber(batteryData?.cell_wire_resistance_status?.[1]),
  battery_bms_cell_wire_status_2: boolToNumber(batteryData?.cell_wire_resistance_status?.[2]),
  battery_bms_cell_wire_status_3: boolToNumber(batteryData?.cell_wire_resistance_status?.[3]),
  battery_bms_cell_wire_status_4: boolToNumber(batteryData?.cell_wire_resistance_status?.[4]),
  battery_bms_cell_wire_status_5: boolToNumber(batteryData?.cell_wire_resistance_status?.[5]),
  battery_bms_cell_wire_status_6: boolToNumber(batteryData?.cell_wire_resistance_status?.[6]),
  battery_bms_cell_wire_status_7: boolToNumber(batteryData?.cell_wire_resistance_status?.[7]),
  battery_bms_cell_wire_status_8: boolToNumber(batteryData?.cell_wire_resistance_status?.[8]),
  battery_bms_cell_wire_status_9: boolToNumber(batteryData?.cell_wire_resistance_status?.[9]),
  battery_bms_cell_wire_status_10: boolToNumber(batteryData?.cell_wire_resistance_status?.[10]),
  battery_bms_cell_wire_status_11: boolToNumber(batteryData?.cell_wire_resistance_status?.[11]),
  battery_bms_cell_wire_status_12: boolToNumber(batteryData?.cell_wire_resistance_status?.[12]),
  battery_bms_cell_wire_status_13: boolToNumber(batteryData?.cell_wire_resistance_status?.[13]),
  battery_bms_cell_wire_status_14: boolToNumber(batteryData?.cell_wire_resistance_status?.[14]),
  battery_bms_cell_wire_status_15: boolToNumber(batteryData?.cell_wire_resistance_status?.[15]),
  battery_bms_cell_wire_status_16: boolToNumber(batteryData?.cell_wire_resistance_status?.[16]),
  battery_bms_cell_wire_status_17: boolToNumber(batteryData?.cell_wire_resistance_status?.[17]),
  battery_bms_cell_wire_status_18: boolToNumber(batteryData?.cell_wire_resistance_status?.[18]),
  battery_bms_cell_wire_status_19: boolToNumber(batteryData?.cell_wire_resistance_status?.[19]),
  battery_bms_cell_wire_status_20: boolToNumber(batteryData?.cell_wire_resistance_status?.[20]),
  battery_bms_cell_wire_status_21: boolToNumber(batteryData?.cell_wire_resistance_status?.[21]),
  battery_bms_cell_wire_status_22: boolToNumber(batteryData?.cell_wire_resistance_status?.[22]),
  battery_bms_cell_wire_status_23: boolToNumber(batteryData?.cell_wire_resistance_status?.[23]),
  battery_bms_cell_wire_status_24: boolToNumber(batteryData?.cell_wire_resistance_status?.[24]),
  battery_bms_cell_wire_status_25: boolToNumber(batteryData?.cell_wire_resistance_status?.[25]),
  battery_bms_cell_wire_status_26: boolToNumber(batteryData?.cell_wire_resistance_status?.[26]),
  battery_bms_cell_wire_status_27: boolToNumber(batteryData?.cell_wire_resistance_status?.[27]),
  battery_bms_cell_wire_status_28: boolToNumber(batteryData?.cell_wire_resistance_status?.[28]),
  battery_bms_cell_wire_status_29: boolToNumber(batteryData?.cell_wire_resistance_status?.[29]),
  battery_bms_cell_wire_status_30: boolToNumber(batteryData?.cell_wire_resistance_status?.[30]),
  battery_bms_cell_wire_status_31: boolToNumber(batteryData?.cell_wire_resistance_status?.[31]),
  battery_bms_alarms_resistence_of_the_balancing_wire_too_large: boolToNumber(batteryData?.alarms.resistence_of_the_balancing_wire_too_large),
  battery_bms_alarms_mos_overtemperature_protection: boolToNumber(batteryData?.alarms.MOS_overtemperature_protection),
  battery_bms_alarms_the_number_of_cells_does_not_match_the_set_value: boolToNumber(batteryData?.alarms.the_number_of_cells_does_not_match_the_set_value),
  battery_bms_alarms_current_sensor_abnormality: boolToNumber(batteryData?.alarms.current_sensor_abnormality),
  battery_bms_alarms_single_unit_overvoltage_protection: boolToNumber(batteryData?.alarms.single_unit_overvoltage_protection),
  battery_bms_alarms_battery_overvoltage_protection: boolToNumber(batteryData?.alarms.battery_overvoltage_protection),
  battery_bms_alarms_charging_overcurrent_protection: boolToNumber(batteryData?.alarms.charging_overcurrent_protection),
  battery_bms_alarms_charging_short_circuit_protection: boolToNumber(batteryData?.alarms.charging_short_circuit_protection),
  battery_bms_alarms_charging_over_temperature_protection: boolToNumber(batteryData?.alarms.charging_over_temperature_protection),
  battery_bms_alarms_charging_low_temperature_protection: boolToNumber(batteryData?.alarms.charging_low_temperature_protection),
  battery_bms_alarms_internal_communication_abnormality: boolToNumber(batteryData?.alarms.internal_communication_abnormality),
  battery_bms_alarms_single_unit_undervoltage_protection: boolToNumber(batteryData?.alarms.single_unit_undervoltage_protection),
  battery_bms_alarms_battery_undervoltage_protection: boolToNumber(batteryData?.alarms.battery_undervoltage_protection),
  battery_bms_alarms_discharge_overcurrent_protection: boolToNumber(batteryData?.alarms.discharge_overcurrent_protection),
  battery_bms_alarms_discharge_short_circuit_protection: boolToNumber(batteryData?.alarms.discharge_short_circuit_protection),
  battery_bms_alarms_discharge_over_temperature_protection: boolToNumber(batteryData?.alarms.discharge_over_temperature_protection),
  battery_bms_alarms_charging_anomaly: boolToNumber(batteryData?.alarms.charging_anomality),
  battery_bms_alarms_discharge_anomaly: boolToNumber(batteryData?.alarms.discharge_anomality),
  battery_bms_alarms_gps_disconnected: boolToNumber(batteryData?.alarms.GPS_disconnected),
  battery_bms_alarms_please_change_the_authorization_password: boolToNumber(batteryData?.alarms.please_change_the_authorization_password),
  battery_bms_alarms_discharge_on_failure: boolToNumber(batteryData?.alarms.discharge_on_failure),
  battery_bms_alarms_battery_over_temperature: boolToNumber(batteryData?.alarms.battery_over_temperature),
  battery_bms_alarms_temperature_sensor_anomaly: boolToNumber(batteryData?.alarms.temperature_sensor_anomaly),
  battery_bms_alarms_parallel_module_failure: boolToNumber(batteryData?.alarms.parallel_module_failure),
  battery_bms_alarms_unknown_1: boolToNumber(batteryData?.alarms.unknown_1),
  battery_bms_alarms_unknown_2: boolToNumber(batteryData?.alarms.unknown_2),
  battery_bms_alarms_unknown_3: boolToNumber(batteryData?.alarms.unknown_3),
  battery_bms_alarms_unknown_4: boolToNumber(batteryData?.alarms.unknown_4),
  battery_bms_alarms_unknown_5: boolToNumber(batteryData?.alarms.unknown_5),
  battery_bms_alarms_unknown_6: boolToNumber(batteryData?.alarms.unknown_6),
  battery_bms_alarms_unknown_7: boolToNumber(batteryData?.alarms.unknown_7),
  battery_bms_alarms_unknown_8: boolToNumber(batteryData?.alarms.unknown_8),
  battery_bms_temperature_sensor_status_mos_temperature_sensor: boolToNumber(batteryData?.temperature_sensor_status.MOS_temperature_sensor),
  battery_bms_temperature_sensor_status_battery_temperature_sensor_1: boolToNumber(batteryData?.temperature_sensor_status.battery_temperature_sensor_1),
  battery_bms_temperature_sensor_status_battery_temperature_sensor_2: boolToNumber(batteryData?.temperature_sensor_status.battery_temperature_sensor_2),
  battery_bms_temperature_sensor_status_battery_temperature_sensor_3: boolToNumber(batteryData?.temperature_sensor_status.battery_temperature_sensor_3),
  battery_bms_temperature_sensor_status_battery_temperature_sensor_4: boolToNumber(batteryData?.temperature_sensor_status.battery_temperature_sensor_4),
  battery_bms_temperature_sensor_status_battery_temperature_sensor_5: boolToNumber(batteryData?.temperature_sensor_status.battery_temperature_sensor_5),
  battery_bms_temperature_sensor_status_unknown1: boolToNumber(batteryData?.temperature_sensor_status.unknown1),
  battery_bms_temperature_sensor_status_unknown2: boolToNumber(batteryData?.temperature_sensor_status.unknown2),
});

const fields = {
	...(systemRes.status === "fulfilled" ? {
		field1: systemRes.value.uptime,
	} : {}),
	...(inverterValuesRes.status === "fulfilled" ? {
		field2: inverterValuesRes.value.qpigs.ac_output_active_power,
		field3: inverterValuesRes.value.qpigs.battery_voltage,
		field4: inverterValuesRes.value.qpigs.battery_charging_current - inverterValuesRes.value.qpigs.battery_discharge_current,
		field7: inverterValuesRes.value.qpigs.pv_charging_power1,
		field8: inverterValuesRes.value.qpigs2.pv_charging_power2,
	} : {}),
	...(bmsValuesRes.status === "fulfilled" ? {
		field5: bmsValuesRes.value.state_of_charge,
		field6: bmsValuesRes.value.cycle_count,
	} : {}),
};
console.log(fields);
const req = await fetch("https://api.thingspeak.com/update" + 
	"?api_key=" + thingspeakKey + Object.entries(fields).map(([k, v]) => `&${k}=${v}`).join(""));
if (!req.ok) {
	console.error(req);
	throw new Error("Could not upload to thingspeak");
}

