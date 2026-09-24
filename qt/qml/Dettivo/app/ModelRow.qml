import QtQuick
import DettivoStyle as Style

Style.ModelChoice {
    Accessible.role: checkable ? Accessible.CheckBox : Accessible.RadioButton
    Accessible.name: accessibleName
}
