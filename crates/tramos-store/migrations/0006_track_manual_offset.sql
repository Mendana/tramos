-- 0006: desfase entre el reloj y el cronometraje fijado a mano para un track (#68). Ver
-- docs/almacenamiento.md y docs/alineacion.md (convenio: instante en el track = picada + desfase).
--
-- NULL = automático: lo estima la alineación. Los tracks guardados antes quedan en automático.
ALTER TABLE tracks ADD COLUMN manual_offset_s REAL;
